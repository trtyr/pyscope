use crate::model::CodeGraph;
use anyhow::Result;
use clap::ValueEnum;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExportFormat {
    Dot,
    Mermaid,
    Json,
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn export(graph: &CodeGraph, format: ExportFormat) -> Result<Value> {
    let content = match format {
        ExportFormat::Json => serde_json::to_string_pretty(graph)?,
        ExportFormat::Dot => {
            let mut out = String::from("digraph pyscope {\n  rankdir=LR;\n");
            for node in &graph.nodes {
                let id = sanitize_id(&node.id);
                let label = escape_dot(&format!("{} [{}]", node.name, node.kind.as_str()));
                out.push_str(&format!("  \"{id}\" [label=\"{label}\"];\n"));
            }
            for edge in &graph.edges {
                let from = sanitize_id(&edge.from);
                let to = sanitize_id(&edge.to);
                let label = escape_dot(edge.kind.as_str());
                out.push_str(&format!("  \"{from}\" -> \"{to}\" [label=\"{label}\"];\n"));
            }
            out.push_str("}\n");
            out
        }
        ExportFormat::Mermaid => {
            let mut out = String::from("graph LR\n");
            for node in &graph.nodes {
                let id = sanitize_id(&node.id);
                let label = format!("{} [{}]", node.name, node.kind.as_str());
                out.push_str(&format!("  {id}[\"{label}\"]\n"));
            }
            for edge in &graph.edges {
                let from = sanitize_id(&edge.from);
                let to = sanitize_id(&edge.to);
                let label = edge.kind.as_str();
                out.push_str(&format!("  {from} -->|{label}| {to}\n"));
            }
            out
        }
    };

    let format_str = match format {
        ExportFormat::Dot => "dot",
        ExportFormat::Mermaid => "mermaid",
        ExportFormat::Json => "json",
    };

    Ok(json!({
        "kind": "export",
        "format": format_str,
        "content": content
    }))
}
