use crate::model::{CodeGraph, EdgeKind, Node, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};

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
    "handler",
    "route",
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

fn has_incoming_calls(index: &QueryIndex<'_>, node: &Node) -> bool {
    index
        .edges(&node.id, false)
        .iter()
        .any(|edge| matches!(edge.kind, EdgeKind::Calls | EdgeKind::AwaitCalls))
}

pub fn entries(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);

    let mut detected: Vec<(&Node, usize)> = graph
        .nodes
        .iter()
        .filter(|node| is_function_like(&node.kind))
        .filter(|node| !has_incoming_calls(&index, node) || looks_like_entry_name(&node.name))
        .map(|node| (node, index.degree(&node.id)))
        .collect();

    detected.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.qualified_name.cmp(&b.0.qualified_name))
    });

    let total = detected.len();
    let entries = detected
        .into_iter()
        .take(limit)
        .map(|(node, degree)| {
            json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
                "file": node.file,
                "range": node.range,
                "degree": degree,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "kind": "entries",
        "entries": entries,
        "total": total,
    })
}
