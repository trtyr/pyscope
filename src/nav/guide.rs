use crate::model::{CodeGraph, EdgeKind, Node, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};
use std::collections::HashSet;

const ENTRY_NAMES: &[&str] = &[
    "main",
    "__main__",
    "cli",
    "app",
    "server",
    "create_app",
    "run",
    "start",
    "setup",
];

fn is_function_like(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::AsyncFunction
            | NodeKind::Method
            | NodeKind::AsyncMethod
            | NodeKind::ClassMethod
            | NodeKind::StaticMethod
    )
}

fn looks_like_entry_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    ENTRY_NAMES.iter().any(|pattern| {
        lowered == *pattern
            || lowered.ends_with(&format!("::{pattern}"))
            || lowered.contains(&format!("_{pattern}"))
    })
}

fn has_incoming_entry_edges(index: &QueryIndex<'_>, node: &Node) -> bool {
    index.edges(&node.id, false).iter().any(|edge| {
        matches!(
            edge.kind,
            EdgeKind::Calls | EdgeKind::AwaitCalls | EdgeKind::Imports
        )
    })
}

fn collect_call_chain(
    index: &QueryIndex<'_>,
    node_id: &str,
    depth: usize,
    seen: &mut HashSet<String>,
    chain: &mut Vec<String>,
) {
    if depth == 0 {
        return;
    }

    for edge in index.edges(node_id, true) {
        if !matches!(edge.kind, EdgeKind::Calls | EdgeKind::AwaitCalls) {
            continue;
        }
        let Some(target) = index.node(&edge.to) else {
            continue;
        };
        if !seen.insert(target.id.clone()) {
            continue;
        }
        chain.push(target.qualified_name.clone());
        collect_call_chain(index, &target.id, depth - 1, seen, chain);
    }
}

pub fn guide(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);

    let mut detected: Vec<(&Node, usize)> = graph
        .nodes
        .iter()
        .filter(|node| is_function_like(&node.kind))
        .filter(|node| !has_incoming_entry_edges(&index, node) || looks_like_entry_name(&node.name))
        .map(|node| (node, index.degree(&node.id)))
        .collect();

    detected.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.qualified_name.cmp(&b.0.qualified_name))
    });

    let entries = detected
        .into_iter()
        .take(limit)
        .map(|(node, _degree)| {
            let mut seen = HashSet::from([node.id.clone()]);
            let mut call_chain = Vec::new();
            collect_call_chain(&index, &node.id, 3, &mut seen, &mut call_chain);

            json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
                "file": node.file,
                "call_chain": call_chain,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "kind": "guide",
        "entries": entries,
    })
}
