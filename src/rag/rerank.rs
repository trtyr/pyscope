use crate::config;
use crate::model::{CodeGraph, Node};
use anyhow::Result;
use serde_json::{Value, json};

use super::embedding::{embed_nodes, embed_query};
use super::lexical::lexical_search;
use super::{ScoredNode, embedding_enabled, node_json};

pub fn rerank(embedded_query: &[f32], nodes: &[(Node, Vec<f32>)], limit: usize) -> Vec<(Node, f32)> {
    let mut items = nodes
        .iter()
        .map(|(node, vector)| (node.clone(), cosine_similarity(embedded_query, vector)))
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.qualified_name.cmp(&b.0.qualified_name))
    });
    items.truncate(limit);
    items
}

pub fn hybrid_search(graph: &CodeGraph, query: &str, limit: usize) -> Result<Value> {
    let lexical_limit = limit.max(1) * 5;
    let lexical = lexical_search(graph, query, lexical_limit);

    let config = config::load()?;
    if !embedding_enabled(&config) {
        return Ok(json!({
            "kind": "retrieve",
            "query": query,
            "strategy": "lexical",
            "items": lexical.into_iter().take(limit).map(scored_value).collect::<Vec<_>>(),
        }));
    }

    let embedding_key = config.embedding_key.as_deref().unwrap_or_default();
    let embedded_query = embed_query(embedding_key, &config.embedding_model, query)?;
    let lexical_nodes = lexical.into_iter().map(|item| item.node).collect::<Vec<_>>();
    let embedded_nodes = embed_nodes(embedding_key, &config.embedding_model, &lexical_nodes)?;
    let reranked = rerank(&embedded_query, &embedded_nodes, limit);

    Ok(json!({
        "kind": "retrieve",
        "query": query,
        "strategy": "hybrid",
        "items": reranked.into_iter().map(|(node, score)| json!({
            "score": score,
            "source": "embedding",
            "node": node_json(&node),
        })).collect::<Vec<_>>(),
    }))
}

fn scored_value(item: ScoredNode) -> Value {
    json!({
        "score": item.score,
        "source": item.source,
        "node": node_json(&item.node),
    })
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}
