use crate::model::CodeGraph;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;

use super::find::{find_nodes, suggest};

pub fn source(graph: &CodeGraph, name: &str) -> Result<Value> {
    let matches = find_nodes(graph, name);
    if matches.is_empty() {
        let names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
        anyhow::bail!("symbol `{name}` not found{}", suggest(name, &names, 3));
    }
    if matches.len() > 1 {
        return Ok(json!({
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
        }));
    }

    let node = matches[0];
    let file = node
        .file
        .as_deref()
        .with_context(|| format!("symbol `{name}` has no associated file"))?;
    let range = node
        .range
        .as_ref()
        .with_context(|| format!("symbol `{name}` has no source range"))?;

    let content = fs::read_to_string(file)
        .with_context(|| format!("failed to read source file `{file}`"))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = range.start_line.saturating_sub(1);
    let end = range.end_line.min(lines.len());
    if start >= end {
        anyhow::bail!(
            "invalid range for `{name}`: start_line={} end_line={} (file has {} lines)",
            range.start_line,
            range.end_line,
            lines.len()
        );
    }
    let snippet: Vec<String> = lines[start..end].iter().map(|s| s.to_string()).collect();

    Ok(json!({
        "kind": "source",
        "node": {
            "id": node.id,
            "name": node.name,
            "qualified_name": node.qualified_name,
            "kind": node.kind.as_str(),
            "file": node.file,
            "range": node.range
        },
        "content": snippet.join("\n"),
        "line_count": snippet.len()
    }))
}
