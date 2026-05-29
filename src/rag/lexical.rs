use crate::model::{CodeGraph, NodeKind};

use super::{ScoredNode, node_text};

pub fn lexical_search(graph: &CodeGraph, query: &str, limit: usize) -> Vec<ScoredNode> {
    let terms = terms(query);
    let mut items = graph
        .nodes
        .iter()
        .filter(|node| !matches!(node.kind, NodeKind::Project | NodeKind::File))
        .filter_map(|node| {
            let text = node_text(node).to_lowercase();
            let overlap = terms
                .iter()
                .filter(|term| text.contains(term.as_str()))
                .count();
            if overlap == 0 {
                return None;
            }
            let exact_name_bonus = if node.name.eq_ignore_ascii_case(query) {
                2.0
            } else {
                0.0
            };
            let score = overlap as f32 + exact_name_bonus;
            Some(ScoredNode {
                node: node.clone(),
                score,
                source: "lexical",
            })
        })
        .collect::<Vec<_>>();

    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.node.qualified_name.cmp(&b.node.qualified_name))
    });
    items.truncate(limit);
    items
}

fn terms(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|term| !term.is_empty())
        .map(ToString::to_string)
        .collect()
}
