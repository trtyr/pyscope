use crate::model::{CodeGraph, EdgeCertainty, NodeKind};
use serde_json::{Value, json};
use std::collections::HashSet;

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

pub fn quality(graph: &CodeGraph) -> Value {
    let total_edges = graph.edges.len();
    let definite_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.certainty == EdgeCertainty::Definite)
        .count();

    let file_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .collect();
    let files_indexed = file_nodes.len();

    let mut files_with_symbols = HashSet::new();
    let mut symbol_nodes = Vec::new();
    let mut nodes_with_docs = 0;
    let mut nodes_with_signature = 0;

    for node in &graph.nodes {
        if matches!(node.kind, NodeKind::File | NodeKind::Project) {
            continue;
        }
        symbol_nodes.push(node);
        if node
            .docs
            .as_deref()
            .is_some_and(|docs| !docs.trim().is_empty())
        {
            nodes_with_docs += 1;
        }
        if node
            .signature
            .as_deref()
            .is_some_and(|signature| !signature.trim().is_empty())
        {
            nodes_with_signature += 1;
        }
        if let Some(file) = node.file.as_deref() {
            files_with_symbols.insert(file.to_string());
        }
    }

    let total_symbol_nodes = symbol_nodes.len();
    let edge_confidence = ratio(definite_edges, total_edges);
    let coverage = ratio(files_with_symbols.len(), files_indexed);
    let documentation = ratio(nodes_with_docs, total_symbol_nodes);
    let type_hints = ratio(nodes_with_signature, total_symbol_nodes);
    let score =
        (edge_confidence * 30.0 + coverage * 30.0 + documentation * 20.0 + type_hints * 20.0)
            .round() as usize;

    json!({
        "kind": "quality",
        "score": score,
        "metrics": {
            "edge_confidence": edge_confidence,
            "coverage": coverage,
            "documentation": documentation,
            "type_hints": type_hints,
            "total_nodes": graph.nodes.len(),
            "total_edges": total_edges,
            "nodes_with_docs": nodes_with_docs,
            "nodes_with_signature": nodes_with_signature,
            "definite_edges": definite_edges,
            "files_indexed": files_indexed,
            "files_with_symbols": files_with_symbols.len(),
        }
    })
}
