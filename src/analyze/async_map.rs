use crate::model::{CodeGraph, EdgeKind, Node, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};

fn is_async_kind(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::AsyncFunction | NodeKind::AsyncMethod)
}

fn is_blocking_name(node: &Node) -> bool {
    let qualified = node.qualified_name.to_ascii_lowercase();
    let name = node.name.to_ascii_lowercase();

    qualified.contains("time.sleep")
        || qualified.contains("requests.")
        || qualified.contains("urllib")
        || qualified.contains("subprocess")
        || qualified.contains("os.system")
        || qualified.contains("socket")
        || qualified.contains("blocking")
        || name == "open"
        || name.ends_with(".open")
        || name == "input"
        || qualified.ends_with(".open")
        || qualified.ends_with(".input")
}

pub fn async_map(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);
    let mut async_nodes: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| is_async_kind(&node.kind))
        .collect();

    async_nodes.sort_by(|a, b| {
        index
            .degree(b.id.as_str())
            .cmp(&index.degree(a.id.as_str()))
            .then_with(|| a.qualified_name.cmp(&b.qualified_name))
    });

    let total_async = async_nodes.len();
    let mut sync_calls: Vec<Value> = Vec::new();
    let mut blocking_calls: Vec<Value> = Vec::new();
    let mut async_functions: Vec<Value> = Vec::new();

    for node in &async_nodes {
        let mut calls_sync = false;

        for edge in index.edges(node.id.as_str(), true) {
            if edge.kind != EdgeKind::Calls {
                continue;
            }

            let Some(callee) = index.node(&edge.to) else {
                continue;
            };

            if is_async_kind(&callee.kind) {
                continue;
            }

            calls_sync = true;
            sync_calls.push(json!({
                "caller": node.qualified_name,
                "callee": callee.qualified_name,
                "callee_file": callee.file,
            }));

            if is_blocking_name(callee) {
                blocking_calls.push(json!({
                    "async_fn": node.qualified_name,
                    "blocking_callee": callee.qualified_name,
                    "file": callee.file,
                }));
            }
        }

        async_functions.push(json!({
            "id": node.id,
            "kind": node.kind.as_str(),
            "qualified_name": node.qualified_name,
            "file": node.file,
            "calls_sync": calls_sync,
        }));
    }

    let total_sync_called_from_async = sync_calls.len();
    let potential_blocking = blocking_calls.len();

    if async_functions.len() > limit {
        async_functions.truncate(limit);
    }
    if sync_calls.len() > limit {
        sync_calls.truncate(limit);
    }
    if blocking_calls.len() > limit {
        blocking_calls.truncate(limit);
    }

    json!({
        "kind": "async_map",
        "async_functions": async_functions,
        "sync_functions_called_from_async": sync_calls,
        "blocking_calls_in_async": blocking_calls,
        "summary": {
            "total_async": total_async,
            "total_sync_called_from_async": total_sync_called_from_async,
            "potential_blocking": potential_blocking,
        }
    })
}
