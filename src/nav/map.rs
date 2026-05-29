use crate::model::{CodeGraph, NodeKind};
use crate::query::index::QueryIndex;
use anyhow::Result;
use serde_json::{Value, json};

/// UTF-8 safe truncation with `...` suffix when truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

/// Format a node's range as `start-end` or `start`.
fn format_range(node: &crate::model::Node) -> String {
    match &node.range {
        Some(r) if r.start_line != r.end_line => {
            format!("{}:{}", r.start_line, r.end_line)
        }
        Some(r) => format!("{}", r.start_line),
        None => "?".to_string(),
    }
}

/// Format signature line if present.
fn format_sig(sig: &str) -> String {
    format!("\n  sig: {}", truncate(sig, 120))
}

/// Format first line of docs if present.
fn format_docs(docs: &str) -> String {
    let first_line = docs.lines().next().unwrap_or(docs);
    format!("\n  docs: {}", truncate(first_line, 120))
}

/// Build a single hot-symbol line.
fn format_hot_symbol(node: &crate::model::Node, deg: usize) -> String {
    let file = node.file.as_deref().unwrap_or("?");
    let range = format_range(node);
    let mut line = format!(
        "- {} `{}` in {}:{} degree={}",
        node.kind.as_str(),
        node.qualified_name,
        file,
        range,
        deg,
    );
    if let Some(sig) = &node.signature {
        line.push_str(&format_sig(sig));
    }
    if let Some(docs) = &node.docs {
        line.push_str(&format_docs(docs));
    }
    line
}

/// Generates a token-budgeted Markdown overview for AI agents.
pub fn nav_map(graph: &CodeGraph, full: bool, budget: usize) -> Result<Value> {
    let index = QueryIndex::new(graph);

    // Project name from root path
    let project_name = graph
        .project
        .root
        .rsplit('/')
        .next()
        .unwrap_or(&graph.project.root);

    let file_count = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .count();

    let mut out = String::new();
    out.push_str("# Codegraph Map\n\n");
    out.push_str(&format!(
        "Project: `{}`\nNodes: {} Edges: {} Files: {}\n",
        project_name,
        graph.nodes.len(),
        graph.edges.len(),
        file_count,
    ));

    // Hot symbols: top 10 by degree
    let mut nodes_with_deg: Vec<(&crate::model::Node, usize)> = graph
        .nodes
        .iter()
        .map(|n| (n, index.degree(&n.id)))
        .collect();
    nodes_with_deg.sort_by(|a, b| b.1.cmp(&a.1));

    out.push_str("\n## Hot Symbols\n");
    for (node, deg) in nodes_with_deg.iter().take(10) {
        out.push_str(&format_hot_symbol(node, *deg));
        out.push('\n');
    }

    if full {
        // Entry points: top 10 by degree, no docs
        out.push_str("\n## Entry Points\n");
        for (node, deg) in nodes_with_deg.iter().take(10) {
            let file = node.file.as_deref().unwrap_or("?");
            let range = format_range(node);
            out.push_str(&format!(
                "- {} `{}` in {}:{} degree={}",
                node.kind.as_str(),
                node.qualified_name,
                file,
                range,
                deg,
            ));
            if let Some(sig) = &node.signature {
                out.push_str(&format_sig(sig));
            }
            out.push('\n');
        }

        // Feature clusters: group by file
        use std::collections::BTreeMap;
        let mut clusters: BTreeMap<&str, Vec<&crate::model::Node>> = BTreeMap::new();
        for node in &graph.nodes {
            if node.kind == NodeKind::File || node.kind == NodeKind::Project {
                continue;
            }
            if let Some(file) = &node.file {
                clusters.entry(file.as_str()).or_default().push(node);
            }
        }

        out.push_str("\n## Feature Clusters\n");
        let mut cluster_vec: Vec<(&str, usize, usize)> = clusters
            .iter()
            .map(|(file, nodes)| {
                let total_deg: usize = nodes.iter().map(|n| index.degree(&n.id)).sum();
                (*file, nodes.len(), total_deg)
            })
            .collect();
        cluster_vec.sort_by(|a, b| b.2.cmp(&a.2));

        for (file, symbol_count, total_deg) in cluster_vec.iter().take(20) {
            let module = file
                .strip_prefix("src/")
                .unwrap_or(file)
                .split('/')
                .next()
                .unwrap_or(file);
            out.push_str(&format!(
                "- `{}` files=1 symbols={} degree={}\n",
                module, symbol_count, total_deg,
            ));
        }
    }

    // Truncate to budget
    let content = truncate(&out, budget);

    Ok(json!({
        "kind": "map",
        "content": content,
        "budget": budget,
    }))
}
