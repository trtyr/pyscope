use crate::model::{CodeGraph, NodeKind};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn fanout(graph: &CodeGraph, limit: usize) -> Value {

    // 1. Build maps: for each file node, count incoming and outgoing edges
    let file_nodes: Vec<&crate::model::Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .collect();

    let mut fanin_map: BTreeMap<&str, usize> = BTreeMap::new();
    let mut fanout_map: BTreeMap<&str, usize> = BTreeMap::new();

    // Initialize all file nodes with 0
    for node in &file_nodes {
        fanin_map.entry(&node.id).or_insert(0);
        fanout_map.entry(&node.id).or_insert(0);
    }

    // Build node_id -> file_node_id lookup
    let node_to_file: BTreeMap<&str, &str> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            let fid = n.file.as_ref().and_then(|_| {
                // Find the file node that owns this node
                // A node's file field contains the path; match it to a File node
                graph.nodes.iter().find(|fnode| {
                    fnode.kind == NodeKind::File
                        && n.file.as_deref() == Some(fnode.name.as_str())
                }).map(|f| f.id.as_str())
            });
            fid.map(|f| (n.id.as_str(), f))
        })
        .collect();

    // 2. Count edges per file
    for edge in &graph.edges {
        let from_file = node_to_file.get(edge.from.as_str());
        let to_file = node_to_file.get(edge.to.as_str());

        // Skip edges within the same file
        match (from_file, to_file) {
            (Some(from_f), Some(to_f)) => {
                if from_f == to_f {
                    continue;
                }
                *fanout_map.entry(from_f).or_insert(0) += 1;
                *fanin_map.entry(to_f).or_insert(0) += 1;
            }
            (Some(from_f), None) => {
                *fanout_map.entry(from_f).or_insert(0) += 1;
            }
            (None, Some(to_f)) => {
                *fanin_map.entry(to_f).or_insert(0) += 1;
            }
            (None, None) => {}
        }
    }

    // 3. Build items with total, sort descending, take top `limit`
    let mut items: Vec<Value> = file_nodes
        .iter()
        .map(|node| {
            let fi = fanin_map.get(node.id.as_str()).copied().unwrap_or(0);
            let fo = fanout_map.get(node.id.as_str()).copied().unwrap_or(0);
            json!({
                "file": node.name,
                "fanin": fi,
                "fanout": fo,
                "total": fi + fo,
            })
        })
        .collect();

    items.sort_by(|a, b| {
        b["total"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["total"].as_u64().unwrap_or(0))
    });
    items.truncate(limit);

    json!({
        "kind": "fanout",
        "items": items,
    })
}
