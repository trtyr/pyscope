use crate::model::{CodeGraph, EdgeKind, NodeKind};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;

use super::filter::SymbolFilter;
use super::find::{find_nodes, require_unique_node, suggest};
use super::index::QueryIndex;
use super::similar;
use super::traversal::{adjacent, node_value, walk};

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum TraceDirection {
    Up,
    Down,
    Both,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum FindMode {
    Text,
    Similar,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ScopeKind {
    File,
    Module,
}

/// Simple text search: filter nodes whose name contains query (case-insensitive),
/// sort by degree descending, take limit.
pub fn search(graph: &CodeGraph, query: &str, limit: usize) -> Value {
    let index = QueryIndex::new(graph);
    let query_lower = query.to_lowercase();
    let mut items: Vec<(&crate::model::Node, usize)> = graph
        .nodes
        .iter()
        .filter(|node| node.name.to_lowercase().contains(&query_lower))
        .map(|node| (node, index.degree(&node.id)))
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    json!({
        "kind": "search",
        "query": query,
        "items": items
            .into_iter()
            .take(limit)
            .map(|(node, _)| node_value(&index, node))
            .collect::<Vec<_>>()
    })
}

pub fn inspect(graph: &CodeGraph, name: &str, include_source: bool) -> Result<Value> {
    let index = QueryIndex::new(graph);
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
    let mut value = json!({
        "kind": "inspect",
        "node": node_value(&index, node),
        "incoming": adjacent(&index, &node.id, false, None, 100),
        "outgoing": adjacent(&index, &node.id, true, None, 100)
    });

    if include_source {
        if let Some(file) = node.file.as_deref() {
            if let Some(range) = &node.range {
                if let Ok(content) = fs::read_to_string(file) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = range.start_line.saturating_sub(1);
                    let end = range.end_line.min(lines.len());
                    if start < end {
                        let snippet: Vec<String> =
                            lines[start..end].iter().map(|s| s.to_string()).collect();
                        value["source"] = json!({
                            "content": snippet.join("\n"),
                            "range": range,
                            "line_count": snippet.len()
                        });
                    }
                }
            }
        }
    }

    Ok(value)
}

pub fn trace(
    graph: &CodeGraph,
    name: &str,
    direction: TraceDirection,
    depth: usize,
    limit: usize,
) -> Result<Value> {
    let index = QueryIndex::new(graph);
    let node = require_unique_node(graph, name, "symbol")?;

    let value = match direction {
        TraceDirection::Up => {
            let items = walk(&index, &node.id, false, Some("calls"), depth, limit);
            json!({
                "kind": "trace",
                "direction": "up",
                "root": node_value(&index, node),
                "depth": depth,
                "items": items
            })
        }
        TraceDirection::Down => {
            let items = walk(&index, &node.id, true, Some("calls"), depth, limit);
            json!({
                "kind": "trace",
                "direction": "down",
                "root": node_value(&index, node),
                "depth": depth,
                "items": items
            })
        }
        TraceDirection::Both => {
            let upstream = walk(&index, &node.id, false, Some("calls"), depth, limit);
            let downstream = walk(&index, &node.id, true, Some("calls"), depth, limit);
            json!({
                "kind": "trace",
                "direction": "both",
                "root": node_value(&index, node),
                "depth": depth,
                "upstream": upstream,
                "downstream": downstream
            })
        }
    };

    Ok(value)
}

pub fn find(graph: &CodeGraph, pattern: &str, mode: FindMode, limit: usize) -> Value {
    match mode {
        FindMode::Text => search(graph, pattern, limit),
        FindMode::Similar => similar::similar(graph, pattern, limit),
    }
}

pub fn symbols(graph: &CodeGraph, kind: Option<&str>, limit: usize, filter: SymbolFilter) -> Value {
    let index = QueryIndex::new(graph);
    let items: Vec<Value> = graph
        .nodes
        .iter()
        .filter(|node| kind.is_none_or(|k| node.kind.as_str() == k))
        .filter(|node| filter.matches(node, &index))
        .take(limit)
        .map(|node| node_value(&index, node))
        .collect();
    json!({
        "kind": "symbols",
        "count": items.len(),
        "applied_filters": filter.description(),
        "items": items
    })
}

pub fn impact(
    graph: &CodeGraph,
    name: &str,
    depth: usize,
    limit: usize,
) -> Result<Value> {
    let index = QueryIndex::new(graph);
    let node = require_unique_node(graph, name, "symbol")?;

    // Upstream callers (walk incoming Calls)
    let callers = walk(&index, &node.id, false, Some("calls"), depth, limit);

    // Downstream dependencies (all outgoing edges)
    let dependencies = walk(&index, &node.id, true, None, depth, limit);

    // Upstream dependents (all incoming edges)
    let dependents = walk(&index, &node.id, false, None, depth, limit);

    // Direct callers for call_sites (depth=1)
    let direct_callers = index
        .edges(&node.id, false)
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect::<Vec<_>>();

    // files_affected: group callers by file, dedup names, sort by count desc
    let mut files_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &direct_callers {
        if let Some(caller_node) = index.node(&edge.from) {
            if let Some(ref file) = caller_node.file {
                files_map
                    .entry(file.clone())
                    .or_default()
                    .push(caller_node.name.clone());
            }
        }
    }
    let mut files_affected: Vec<Value> = files_map
        .into_iter()
        .map(|(file, mut names)| {
            names.sort();
            names.dedup();
            let count = names.len();
            json!({
                "file": file,
                "count": count,
                "symbols": names.into_iter().take(5).collect::<Vec<_>>()
            })
        })
        .collect();
    files_affected.sort_by(|a, b| {
        b["count"].as_u64().unwrap_or(0).cmp(&a["count"].as_u64().unwrap_or(0))
    });

    // call_sites: direct callers with call_style and evidence
    let call_sites: Vec<Value> = direct_callers
        .iter()
        .filter_map(|edge| {
            let caller = index.node(&edge.from)?;
            Some(json!({
                "caller": caller.name,
                "qualified_name": caller.qualified_name,
                "call_style": edge.call_style,
                "evidence": edge.evidence,
                "at_line": edge.evidence.as_ref().map(|e| e.line)
            }))
        })
        .collect();

    // change_hints: heuristic analysis
    let mut change_hints = Vec::new();
    if direct_callers.is_empty() {
        change_hints.push("Safe to remove — no callers found".to_string());
    } else {
        let unique_files: HashSet<&str> = direct_callers
            .iter()
            .filter_map(|e| index.node(&e.from))
            .filter_map(|n| n.file.as_deref())
            .collect();
        if unique_files.len() >= 3 {
            change_hints.push(format!(
                "High: changes propagate to {} files",
                unique_files.len()
            ));
        } else {
            change_hints.push(format!(
                "Low: contained change ({} file(s))",
                unique_files.len()
            ));
        }
        if node.visibility.as_deref() == Some("public") {
            change_hints.push(
                "Consider deprecation period — symbol is public and has callers".to_string(),
            );
        }
        let has_method_call = direct_callers
            .iter()
            .any(|e| e.call_style.as_deref() == Some("method"));
        if has_method_call {
            change_hints.push(
                "Check class hierarchy for breaking changes — method call style detected"
                    .to_string(),
            );
        }
    }

    Ok(json!({
        "kind": "impact",
        "root": node_value(&index, node),
        "callers": callers,
        "dependents": dependents,
        "dependencies": dependencies,
        "files_affected": files_affected,
        "call_sites": call_sites,
        "change_hints": change_hints
    }))
}

pub fn scope(graph: &CodeGraph, target: &str, kind: ScopeKind) -> Result<Value> {
    match kind {
        ScopeKind::File => {
            let index = QueryIndex::new(graph);
            let node = graph
                .nodes
                .iter()
                .find(|node| {
                    node.kind == NodeKind::File
                        && (node.name == target
                            || node.name.ends_with(target)
                            || node.qualified_name.ends_with(target))
                })
                .with_context(|| {
                    let files: Vec<&str> = graph
                        .nodes
                        .iter()
                        .filter(|n| n.kind == NodeKind::File)
                        .map(|n| n.name.as_str())
                        .collect();
                    format!(
                        "file `{target}` not found{}",
                        suggest(target, &files, 3)
                    )
                })?;
            // Follow module_file edge to get the module, then get its declares
            let declares = index
                .edges(&node.id, true)
                .iter()
                .find(|edge| edge.kind == EdgeKind::ModuleFile)
                .map(|mf| adjacent(&index, &mf.to, true, Some("declares"), 500))
                .unwrap_or_default();
            Ok(json!({
                "kind": "scope",
                "node": node_value(&index, node),
                "declares": declares,
                "incoming": adjacent(&index, &node.id, false, None, 100),
                "outgoing": adjacent(&index, &node.id, true, None, 100)
            }))
        }
        ScopeKind::Module => {
            let index = QueryIndex::new(graph);
            let matches = find_nodes(graph, target)
                .into_iter()
                .filter(|node| node.kind == NodeKind::Module)
                .collect::<Vec<_>>();
            let node = matches.first().copied().with_context(|| {
                let mods: Vec<&str> = graph
                    .nodes
                    .iter()
                    .filter(|n| n.kind == NodeKind::Module)
                    .map(|n| n.name.as_str())
                    .collect();
                format!(
                    "module `{target}` not found{}",
                    suggest(target, &mods, 3)
                )
            })?;
            Ok(json!({
                "kind": "scope",
                "node": node_value(&index, node),
                "declares": adjacent(&index, &node.id, true, Some("declares"), 500),
                "imports": adjacent(&index, &node.id, true, Some("imports"), 200),
                "incoming": adjacent(&index, &node.id, false, None, 100)
            }))
        }
    }
}
