use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};

use super::index::QueryIndex;
use crate::model::Node;

pub fn node_value(index: &QueryIndex, node: &Node) -> Value {
    json!({
        "id": node.id,
        "kind": node.kind.as_str(),
        "name": node.name,
        "qualified_name": node.qualified_name,
        "file": node.file,
        "range": node.range,
        "visibility": node.visibility,
        "signature": node.signature,
        "docs": node.docs,
        "degree": index.degree(&node.id)
    })
}

pub fn adjacent(
    index: &QueryIndex,
    id: &str,
    outbound: bool,
    kind: Option<&str>,
    limit: usize,
) -> Vec<Value> {
    index
        .edges(id, outbound)
        .iter()
        .copied()
        .filter(|edge| kind.is_none_or(|kind| edge.kind.as_str() == kind))
        .filter_map(|edge| {
            let other = if outbound { &edge.to } else { &edge.from };
            index.node(other).map(|node| {
                json!({
                    "edge": edge,
                    "node": node_value(index, node)
                })
            })
        })
        .take(limit)
        .collect()
}

pub fn walk(
    index: &QueryIndex,
    start: &str,
    outbound: bool,
    kind: Option<&str>,
    depth: usize,
    limit: usize,
) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([(start.to_string(), 0usize)]);
    let mut result = Vec::new();
    while let Some((id, level)) = queue.pop_front() {
        if level >= depth || result.len() >= limit {
            continue;
        }
        for edge in index
            .edges(&id, outbound)
            .iter()
            .copied()
            .filter(|edge| kind.is_none_or(|kind| edge.kind.as_str() == kind))
        {
            let other = if outbound { &edge.to } else { &edge.from };
            if !seen.insert(other.clone()) {
                continue;
            }
            if let Some(node) = index.node(other) {
                result.push(json!({
                    "depth": level + 1,
                    "edge": edge,
                    "node": node_value(index, node)
                }));
                queue.push_back((other.clone(), level + 1));
            }
        }
    }
    result
}

/// BFS-based shortest path between two symbols following Calls edges.
/// Takes `&CodeGraph` to resolve names, then searches by node IDs.
pub fn shortest_path(
    graph: &crate::model::CodeGraph,
    from: &str,
    to: &str,
    max_depth: usize,
) -> Value {
    use std::collections::VecDeque;

    let index = QueryIndex::new(graph);

    // Resolve names to IDs
    let from_nodes = super::find::find_nodes(graph, from);
    let to_nodes = super::find::find_nodes(graph, to);

    if from_nodes.is_empty() || to_nodes.is_empty() {
        return json!({
            "kind": "path",
            "found": false,
            "from": from,
            "to": to
        });
    }

    let from_id = &from_nodes[0].id;
    let to_set: std::collections::BTreeSet<&str> = to_nodes.iter().map(|n| n.id.as_str()).collect();

    // BFS following outgoing calls edges
    let mut queue: VecDeque<(String, Vec<String>)> =
        VecDeque::from([(from_id.clone(), vec![from_id.clone()])]);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(from_id.clone());

    while let Some((id, path)) = queue.pop_front() {
        if path.len() > max_depth {
            continue;
        }
        if to_set.contains(id.as_str()) {
            let nodes: Vec<Value> = path
                .iter()
                .filter_map(|id| index.node(id))
                .map(|n| node_value(&index, n))
                .collect();
            return json!({
                "kind": "path",
                "found": true,
                "length": path.len().saturating_sub(1),
                "path": nodes,
                "from": from,
                "to": to
            });
        }
        for edge in index.edges(&id, true) {
            if edge.kind.as_str() != "calls" {
                continue;
            }
            let other = &edge.to;
            if !seen.insert(other.clone()) {
                continue;
            }
            let mut new_path = path.clone();
            new_path.push(other.clone());
            queue.push_back((other.clone(), new_path));
        }
    }

    json!({
        "kind": "path",
        "found": false,
        "from": from,
        "to": to
    })
}
