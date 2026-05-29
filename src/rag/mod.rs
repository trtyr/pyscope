pub mod embedding;
pub mod lexical;
pub mod rerank;

use crate::config;
use crate::model::{CodeGraph, Node};
use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize)]
pub struct ScoredNode {
    pub node: Node,
    pub score: f32,
    pub source: &'static str,
}

pub fn retrieve(graph: &CodeGraph, query: &str, limit: usize) -> Result<Value> {
    rerank::hybrid_search(graph, query, limit)
}

pub(crate) fn node_text(node: &Node) -> String {
    [
        node.name.as_str(),
        node.qualified_name.as_str(),
        node.signature.as_deref().unwrap_or_default(),
        node.docs.as_deref().unwrap_or_default(),
        node.file.as_deref().unwrap_or_default(),
    ]
    .join("\n")
}

pub(crate) fn node_json(node: &Node) -> Value {
    json!({
        "id": node.id,
        "kind": node.kind.as_str(),
        "name": node.name,
        "qualified_name": node.qualified_name,
        "file": node.file,
        "range": node.range,
        "signature": node.signature,
        "docs": node.docs,
    })
}

pub(crate) fn embedding_enabled(config: &config::CodegraphConfig) -> bool {
    config
        .embedding_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty())
}
