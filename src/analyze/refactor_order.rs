use crate::model::{CodeGraph, EdgeKind};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn normalize_module_name(module: &str) -> String {
    module.trim_matches('/').replace('\\', "/")
}

fn file_matches_module(file: &str, module: &str) -> bool {
    let module = normalize_module_name(module);
    file == module
        || file.starts_with(&format!("{module}/"))
        || file.starts_with(&format!("src/{module}"))
        || file.starts_with(&format!("{module}."))
        || file.starts_with(&format!("src/{module}."))
}

fn risk_level(score: usize) -> &'static str {
    match score {
        0 => "none",
        1..=3 => "low",
        4..=8 => "medium",
        _ => "high",
    }
}

pub fn refactor_order(graph: &CodeGraph, modules: &[String], limit: usize) -> Value {
    let target_modules: Vec<String> = modules.iter().map(|m| normalize_module_name(m)).collect();

    let mut module_nodes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for module in &target_modules {
        let nodes = graph
            .nodes
            .iter()
            .filter_map(|node| {
                let file = node.file.as_deref()?;
                if file_matches_module(file, module) {
                    Some(node.id.clone())
                } else {
                    None
                }
            })
            .collect();
        module_nodes.insert(module.clone(), nodes);
    }

    let mut node_to_module: BTreeMap<String, String> = BTreeMap::new();
    for (module, nodes) in &module_nodes {
        for node_id in nodes {
            node_to_module.insert(node_id.clone(), module.clone());
        }
    }

    let relevant_kinds = [
        EdgeKind::Calls,
        EdgeKind::AwaitCalls,
        EdgeKind::Imports,
        EdgeKind::FromImports,
        EdgeKind::UsesType,
        EdgeKind::Declares,
        EdgeKind::Implements,
        EdgeKind::InheritsFrom,
        EdgeKind::Overrides,
    ];

    let mut dependencies: BTreeMap<String, BTreeSet<String>> = target_modules
        .iter()
        .cloned()
        .map(|module| (module, BTreeSet::new()))
        .collect();
    let mut dependents: BTreeMap<String, BTreeSet<String>> = target_modules
        .iter()
        .cloned()
        .map(|module| (module, BTreeSet::new()))
        .collect();

    for edge in &graph.edges {
        if !relevant_kinds.contains(&edge.kind) {
            continue;
        }
        let Some(from_module) = node_to_module.get(&edge.from) else {
            continue;
        };
        let Some(to_module) = node_to_module.get(&edge.to) else {
            continue;
        };
        if from_module == to_module {
            continue;
        }

        dependencies
            .entry(from_module.clone())
            .or_default()
            .insert(to_module.clone());
        dependents
            .entry(to_module.clone())
            .or_default()
            .insert(from_module.clone());
    }

    let mut remaining_dependencies: BTreeMap<String, usize> = target_modules
        .iter()
        .map(|module| {
            (
                module.clone(),
                dependencies
                    .get(module)
                    .map(|set| set.len())
                    .unwrap_or_default(),
            )
        })
        .collect();
    let original_dependencies = remaining_dependencies.clone();

    let mut queue: VecDeque<String> = remaining_dependencies
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(module, _)| module.clone())
        .collect();
    let mut processed = BTreeSet::new();
    let mut ordered_modules = Vec::new();

    while let Some(module) = queue.pop_front() {
        if !processed.insert(module.clone()) {
            continue;
        }

        ordered_modules.push(module.clone());
        if let Some(next_modules) = dependents.get(&module) {
            for dependent in next_modules {
                if let Some(count) = remaining_dependencies.get_mut(dependent) {
                    if *count > 0 {
                        *count -= 1;
                    }
                    if *count == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    let mut order: Vec<Value> = ordered_modules
        .iter()
        .enumerate()
        .map(|(idx, module)| {
            let dependency_list: Vec<String> = dependencies
                .get(module)
                .into_iter()
                .flat_map(|set| set.iter().cloned())
                .collect();
            let indegree = original_dependencies
                .get(module)
                .copied()
                .unwrap_or_default();
            let score = indegree * 2;
            let reason = if dependency_list.is_empty() {
                "No dependencies on other target modules".to_string()
            } else {
                format!("Depends on {}", dependency_list.join(", "))
            };
            json!({
                "step": idx + 1,
                "module": module,
                "risk": {
                    "score": score,
                    "level": risk_level(score),
                },
                "reason": reason,
                "in_degree": indegree,
            })
        })
        .collect();
    order.truncate(limit);

    let mut cycles: Vec<Value> = target_modules
        .iter()
        .filter(|module| !processed.contains(*module))
        .map(|module| {
            let deps: Vec<String> = dependencies
                .get(module)
                .into_iter()
                .flat_map(|set| set.iter().cloned())
                .filter(|dep| !processed.contains(dep))
                .collect();
            json!({
                "module": module,
                "depends_on": deps,
            })
        })
        .collect();
    cycles.truncate(limit);

    json!({
        "kind": "refactor_order",
        "order": order,
        "cycles": cycles,
        "summary": {
            "total_modules": target_modules.len(),
            "has_cycles": processed.len() != target_modules.len(),
        },
    })
}
