use crate::model::{CodeGraph, Node, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Default)]
struct Cluster<'a> {
    files: Vec<&'a str>,
    nodes: Vec<&'a Node>,
    total_degree: usize,
}

fn cluster_key(file: &str) -> String {
    Path::new(file)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| ".".to_string())
}

fn cluster_name(key: &str) -> String {
    Path::new(key)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| key.to_string())
}

pub fn clusters(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);
    let mut grouped: BTreeMap<String, Cluster<'_>> = BTreeMap::new();

    for node in &graph.nodes {
        if matches!(node.kind, NodeKind::File | NodeKind::Project) {
            continue;
        }
        let Some(file) = node.file.as_deref() else {
            continue;
        };
        let entry = grouped.entry(cluster_key(file)).or_default();
        if !entry.files.contains(&file) {
            entry.files.push(file);
        }
        entry.total_degree += index.degree(&node.id);
        entry.nodes.push(node);
    }

    let mut clusters = grouped.into_iter().collect::<Vec<_>>();
    clusters.sort_by(|a, b| {
        b.1.total_degree
            .cmp(&a.1.total_degree)
            .then_with(|| b.1.nodes.len().cmp(&a.1.nodes.len()))
            .then_with(|| a.0.cmp(&b.0))
    });

    let clusters = clusters
        .into_iter()
        .take(limit)
        .map(|(key, mut cluster)| {
            cluster.files.sort_unstable();

            let mut top_symbols: Vec<(&Node, usize)> = cluster
                .nodes
                .iter()
                .map(|node| (*node, index.degree(&node.id)))
                .collect();
            top_symbols.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| a.0.qualified_name.cmp(&b.0.qualified_name))
            });

            json!({
                "name": cluster_name(&key),
                "files": cluster.files,
                "symbol_count": cluster.nodes.len(),
                "total_degree": cluster.total_degree,
                "top_symbols": top_symbols.into_iter().take(5).map(|(node, degree)| json!({
                    "id": node.id,
                    "kind": node.kind.as_str(),
                    "qualified_name": node.qualified_name,
                    "file": node.file,
                    "degree": degree,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "kind": "clusters",
        "clusters": clusters,
    })
}
