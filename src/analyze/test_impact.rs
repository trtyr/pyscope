use crate::model::{CodeGraph, EdgeKind, NodeKind};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::query::find::find_nodes;
use crate::query::index::QueryIndex;
use crate::query::traversal::node_value;

/// Walk parent pointers from `from_id` to `target_id`, collecting IDs, then reverse.
fn build_path(
    target_id: &str,
    from_id: &str,
    parent_map: &BTreeMap<String, String>,
    index: &QueryIndex,
) -> Vec<Value> {
    let mut path = Vec::new();
    let mut current = from_id.to_string();

    // Walk from from_id back to target_id via parent pointers
    while current != target_id {
        if let Some(node) = index.node(&current) {
            path.push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
            }));
        }
        if let Some(parent) = parent_map.get(&current) {
            current = parent.clone();
        } else {
            break;
        }
    }
    // Include the target node
    if let Some(node) = index.node(target_id) {
        path.push(json!({
            "id": node.id,
            "name": node.name,
            "kind": node.kind.as_str(),
        }));
    }

    path.reverse();
    path
}

fn is_test_function(name: &str, file: &Option<String>) -> bool {
    name.starts_with("test_")
        || name.ends_with("_test")
        || file
            .as_deref()
            .is_some_and(|f| f.contains("test") || f.contains("tests"))
}

pub fn test_impact(graph: &CodeGraph, symbol: Option<&str>, limit: usize) -> Value {
    let index = QueryIndex::new(graph);

    let candidate_tests: Vec<Value> = if let Some(sym) = symbol {
        // 1. Resolve the symbol
        let matches = find_nodes(graph, sym);
        if matches.is_empty() {
            return json!({
                "kind": "tests",
                "query": sym,
                "target": null,
                "candidate_tests": [],
                "targets": package_targets(graph),
                "note": format!("symbol `{sym}` not found"),
            });
        }

        let target = matches[0];
        let target_id = target.id.clone();

        // 2. Reverse BFS on call graph (follow incoming Calls edges) up to depth 4
        let mut parent_map: BTreeMap<String, String> = BTreeMap::new();
        let mut visited = BTreeSet::new();
        visited.insert(target_id.clone());
        let mut queue = VecDeque::new();
        queue.push_back((target_id.clone(), 0usize));
        let mut test_hits: Vec<(String, f64)> = Vec::new();

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= 4 {
                continue;
            }
            // Follow incoming Calls edges
            for edge in index.edges(&current_id, false) {
                if edge.kind != EdgeKind::Calls {
                    continue;
                }
                let caller_id = edge.from.clone();
                if !visited.insert(caller_id.clone()) {
                    continue;
                }
                parent_map.insert(caller_id.clone(), current_id.clone());

                if let Some(caller_node) = index.node(&caller_id) {
                    if is_test_function(&caller_node.name, &caller_node.file) {
                        let score = 1.0 / (depth + 1) as f64;
                        test_hits.push((caller_id.clone(), score));
                    }
                }
                queue.push_back((caller_id, depth + 1));
            }
        }

        // Sort by score descending, take limit
        test_hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        test_hits.truncate(limit);

        test_hits
            .into_iter()
            .filter_map(|(test_id, score)| {
                let node = index.node(&test_id)?;
                let path = build_path(&target_id, &test_id, &parent_map, &index);
                Some(json!({
                    "node": node_value(&index, node),
                    "score": score,
                    "path": path,
                }))
            })
            .collect()
    } else {
        // 3. Static discovery: scan all Function/Method nodes for test-like names
        graph
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.kind,
                    NodeKind::Function
                        | NodeKind::Method
                        | NodeKind::AsyncFunction
                        | NodeKind::AsyncMethod
                )
            })
            .filter(|node| is_test_function(&node.name, &node.file))
            .take(limit)
            .map(|node| {
                json!({
                    "node": node_value(&index, node),
                    "score": null,
                    "path": [],
                })
            })
            .collect()
    };

    let note = if symbol.is_some() {
        "call-graph-based test impact analysis"
    } else {
        "static test discovery (no symbol specified)"
    };

    json!({
        "kind": "tests",
        "query": symbol,
        "target": symbol.and_then(|_| find_nodes(graph, symbol.unwrap_or("")).first().map(|n| node_value(&index, n))),
        "candidate_tests": candidate_tests,
        "targets": package_targets(graph),
        "note": note,
    })
}

fn package_targets(graph: &CodeGraph) -> Value {
    let targets: Vec<Value> = graph
        .project
        .packages
        .iter()
        .map(|pkg| {
            json!({
                "name": pkg.name,
                "root": pkg.root,
                "file_count": pkg.files.len(),
            })
        })
        .collect();
    json!(targets)
}
