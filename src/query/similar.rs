use crate::model::{CodeGraph, EdgeKind};
use serde_json::{Value, json};

use super::find::find_nodes;
use super::index::QueryIndex;
use super::traversal::node_value;

pub fn similar(graph: &CodeGraph, name: &str, limit: usize) -> Value {
    let matches = find_nodes(graph, name);
    if matches.is_empty() {
        return json!({
            "kind": "error",
            "message": format!("symbol `{name}` not found")
        });
    }
    if matches.len() > 1 {
        return json!({
            "kind": "ambiguous",
            "name": name,
            "matches": matches.iter().map(|node| json!({
                "id": node.id,
                "name": node.name,
                "qualified_name": node.qualified_name,
                "kind": node.kind.as_str(),
                "file": node.file,
                "range": node.range
            })).collect::<Vec<_>>()
        });
    }

    let target = matches[0];
    let index = QueryIndex::new(graph);

    // Collect callees of the target
    let target_callees: std::collections::HashSet<&str> = graph
        .edges
        .iter()
        .filter(|e| e.from == target.id && e.kind == EdgeKind::Calls)
        .map(|e| e.to.as_str())
        .collect();

    let target_sig_words: Vec<&str> = target
        .signature
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .collect();

    let mut scored: Vec<(&crate::model::Node, usize)> = graph
        .nodes
        .iter()
        .filter(|node| node.id != target.id && node.kind == target.kind)
        .map(|node| {
            // Callee overlap
            let candidate_callees: std::collections::HashSet<&str> = graph
                .edges
                .iter()
                .filter(|e| e.from == node.id && e.kind == EdgeKind::Calls)
                .map(|e| e.to.as_str())
                .collect();
            let callee_overlap = target_callees.intersection(&candidate_callees).count() * 3;

            // Signature keyword overlap
            let candidate_sig_words: std::collections::HashSet<&str> = node
                .signature
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            let keyword_overlap = target_sig_words
                .iter()
                .filter(|w| candidate_sig_words.contains(*w))
                .count();

            let score = callee_overlap + keyword_overlap;
            (node, score)
        })
        .filter(|(_, score)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    json!({
        "kind": "similar",
        "target": node_value(&index, target),
        "items": scored
            .into_iter()
            .take(limit)
            .map(|(node, score)| json!({
                "node": node_value(&index, node),
                "similarity_score": score
            }))
            .collect::<Vec<_>>()
    })
}
