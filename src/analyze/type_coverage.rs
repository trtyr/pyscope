use crate::model::{CodeGraph, Node, NodeKind};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn is_function_like(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::AsyncFunction
            | NodeKind::Method
            | NodeKind::AsyncMethod
            | NodeKind::ClassMethod
            | NodeKind::StaticMethod
            | NodeKind::Property
            | NodeKind::Generator
            | NodeKind::Constructor
            | NodeKind::Dunder
    )
}

fn has_type_annotation(node: &Node) -> bool {
    node.signature
        .as_deref()
        .map(|signature| signature.contains("->") || signature.contains(':'))
        .unwrap_or(false)
}

fn module_key(file: Option<&str>) -> String {
    file.and_then(|path| path.split('/').next())
        .filter(|part| !part.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub fn type_coverage(graph: &CodeGraph, limit: usize) -> Value {
    let mut by_module_counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut untyped_nodes: Vec<&Node> = Vec::new();
    let mut total_functions = 0usize;
    let mut typed_functions = 0usize;

    for node in graph
        .nodes
        .iter()
        .filter(|node| is_function_like(&node.kind))
    {
        total_functions += 1;
        let typed = has_type_annotation(node);
        if typed {
            typed_functions += 1;
        } else {
            untyped_nodes.push(node);
        }

        let entry = by_module_counts
            .entry(module_key(node.file.as_deref()))
            .or_insert((0, 0));
        entry.0 += 1;
        if typed {
            entry.1 += 1;
        }
    }

    let mut by_module: Vec<Value> = by_module_counts
        .into_iter()
        .map(|(module, (total, typed))| {
            let coverage = if total == 0 {
                0.0
            } else {
                typed as f64 / total as f64
            };
            json!({
                "module": module,
                "coverage": coverage,
                "total": total,
                "typed": typed,
            })
        })
        .collect();

    by_module.sort_by(|a, b| {
        a["coverage"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&b["coverage"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b["total"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["total"].as_u64().unwrap_or(0))
            })
            .then_with(|| {
                a["module"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["module"].as_str().unwrap_or(""))
            })
    });

    untyped_nodes.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
    let untyped_functions: Vec<Value> = untyped_nodes
        .into_iter()
        .take(limit)
        .map(|node| {
            json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
                "file": node.file,
            })
        })
        .collect();

    let overall = if total_functions == 0 {
        0.0
    } else {
        typed_functions as f64 / total_functions as f64
    };

    json!({
        "kind": "type_coverage",
        "overall": overall,
        "by_module": by_module,
        "untyped_functions": untyped_functions,
        "summary": {
            "total_functions": total_functions,
            "typed_functions": typed_functions,
        }
    })
}
