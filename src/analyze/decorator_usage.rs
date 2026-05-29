use crate::model::{CodeGraph, EdgeKind, Node, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct DecoratorAggregate {
    count: usize,
    files: BTreeSet<String>,
    targets: BTreeSet<String>,
}

fn custom_decorator_name(name: &str) -> bool {
    const COMMON_PREFIXES: [&str; 11] = [
        "property",
        "staticmethod",
        "classmethod",
        "abc.abstractmethod",
        "typing.",
        "dataclasses.",
        "functools.",
        "contextlib.",
        "pytest.",
        "unittest.mock.",
        "django.",
    ];

    !COMMON_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        && !name.starts_with("flask.")
        && !name.starts_with("fastapi.")
}

fn decorator_files(index: &QueryIndex<'_>, node: &Node) -> BTreeSet<String> {
    let mut files = BTreeSet::new();

    for edge in index.edges(node.id.as_str(), false) {
        if edge.kind != EdgeKind::Declares {
            continue;
        }

        if let Some(parent) = index.node(&edge.from) {
            if let Some(file) = parent.file.as_deref() {
                files.insert(file.to_string());
            } else if !parent.name.is_empty() {
                files.insert(parent.name.clone());
            }
        }
    }

    if files.is_empty() {
        if let Some(file) = node.file.as_deref() {
            files.insert(file.to_string());
        }
    }

    files
}

pub fn decorator_usage(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);
    let decorator_nodes: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Decorator)
        .collect();

    let mut aggregates: BTreeMap<String, DecoratorAggregate> = BTreeMap::new();

    for node in decorator_nodes {
        let key = if node.qualified_name.is_empty() {
            node.name.clone()
        } else {
            node.qualified_name.clone()
        };
        let entry = aggregates.entry(key).or_default();
        entry.count += 1;

        for file in decorator_files(&index, node) {
            entry.files.insert(file);
        }

        for edge in index.edges(node.id.as_str(), true) {
            if edge.kind != EdgeKind::Decorates {
                continue;
            }
            if let Some(target) = index.node(&edge.to) {
                entry.targets.insert(target.qualified_name.clone());
            }
        }
    }

    let total_decorators = aggregates.len();
    let total_applications: usize = aggregates.values().map(|entry| entry.count).sum();

    let mut sorted: Vec<(String, DecoratorAggregate)> = aggregates.into_iter().collect();
    sorted.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));

    let decorators: Vec<Value> = sorted
        .iter()
        .take(limit)
        .map(|(name, entry)| {
            json!({
                "name": format!("@{name}"),
                "count": entry.count,
                "files": entry.files.iter().cloned().collect::<Vec<_>>(),
                "targets": entry.targets.iter().cloned().collect::<Vec<_>>(),
            })
        })
        .collect();

    let custom_decorators: Vec<Value> = sorted
        .iter()
        .filter(|(name, _)| custom_decorator_name(name))
        .take(limit)
        .map(|(name, entry)| {
            json!({
                "name": format!("@{name}"),
                "defined_in": entry.files.iter().next().cloned(),
                "usage_count": entry.count,
            })
        })
        .collect();

    json!({
        "kind": "decorator_usage",
        "decorators": decorators,
        "custom_decorators": custom_decorators,
        "summary": {
            "total_decorators": total_decorators,
            "total_applications": total_applications,
            "custom_count": custom_decorators.len(),
        }
    })
}
