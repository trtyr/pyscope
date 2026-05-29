use crate::model::{CodeGraph, EdgeKind};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashSet;

use super::find::require_unique_node;
use super::index::QueryIndex;
use super::traversal::{node_value, walk};

pub fn risk(graph: &CodeGraph, name: &str, depth: usize, limit: usize) -> Result<Value> {
    let index = QueryIndex::new(graph);
    let node = require_unique_node(graph, name, "symbol")?;
    let direct_callers = index
        .edges(&node.id, false)
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls)
        .collect::<Vec<_>>();
    let affected_files = direct_callers
        .iter()
        .filter_map(|edge| index.node(&edge.from))
        .filter_map(|node| node.file.as_deref())
        .collect::<HashSet<_>>();
    let dependencies = walk(&index, &node.id, true, None, depth, limit);
    let dependents = walk(&index, &node.id, false, None, depth, limit);
    let score = risk_score(
        direct_callers.len(),
        affected_files.len(),
        node.visibility.as_deref(),
    );

    Ok(json!({
        "kind": "risk",
        "root": node_value(&index, node),
        "risk": {
            "score": score,
            "level": risk_level(score),
            "direct_callers": direct_callers.len(),
            "affected_files": affected_files.len(),
        },
        "dependents": dependents,
        "dependencies": dependencies,
    }))
}

fn risk_score(callers: usize, files: usize, visibility: Option<&str>) -> usize {
    let visibility_score = match visibility {
        Some("public" | "pub") => 20,
        _ => 0,
    };
    (callers.min(20) * 2 + files.min(10) * 4 + visibility_score).min(100)
}

fn risk_level(score: usize) -> &'static str {
    match score {
        0..=19 => "low",
        20..=49 => "medium",
        50..=79 => "high",
        _ => "critical",
    }
}
