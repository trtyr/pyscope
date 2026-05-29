mod helpers;

use crate::model::{
    CodeGraph, Edge, EdgeCertainty, EdgeKind, EdgeSource, Location, NodeKind, SemanticInfo,
};
use anyhow::{Context, Result};
use helpers::{find_language_server, hover_type, lsp_position, path_to_uri};
use lsp_types::{Hover, ReferenceContext, Url};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub fn enrich(graph: &mut CodeGraph, project: &Path, limit: usize) -> Result<SemanticInfo> {
    let Some(server_path) = find_language_server() else {
        return Ok(SemanticInfo {
            enabled: false,
            provider: String::new(),
            scanned_symbols: 0,
            enriched_symbols: 0,
            confirmed_symbols: 0,
            enriched_edges: 0,
            confirmed_edges: 0,
            unresolved_items: 0,
            warnings: Vec::new(),
        });
    };

    let provider = server_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("python-lsp")
        .to_string();

    let mut server = spawn_server(&server_path)
        .with_context(|| format!("failed to start semantic provider {provider}"))?;

    let mut info = SemanticInfo {
        enabled: true,
        provider,
        scanned_symbols: 0,
        enriched_symbols: 0,
        confirmed_symbols: 0,
        enriched_edges: 0,
        confirmed_edges: 0,
        unresolved_items: 0,
        warnings: Vec::new(),
    };

    let root = project
        .canonicalize()
        .with_context(|| format!("failed to resolve project path {}", project.display()))?;

    if let Err(error) = initialize(&mut server, &root) {
        info.enabled = false;
        info.warnings.push(format!("semantic initialize failed: {error:#}"));
        let _ = server.kill();
        let _ = server.wait();
        return Ok(info);
    }

    let function_kinds = [
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::AsyncFunction,
        NodeKind::AsyncMethod,
        NodeKind::ClassMethod,
        NodeKind::StaticMethod,
        NodeKind::Property,
    ];

    let targets = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| function_kinds.contains(&node.kind))
        .take(limit)
        .map(|(index, node)| (index, node.file.clone(), node.range.clone()))
        .collect::<Vec<_>>();

    let mut opened = HashSet::new();
    let mut ids = 2_u64;

    for (node_index, file, range) in targets {
        info.scanned_symbols += 1;

        let Some(rel_file) = file else {
            info.unresolved_items += 1;
            continue;
        };
        let Some(range) = range else {
            info.unresolved_items += 1;
            continue;
        };

        let abs_file = root.join(&rel_file);
        let source = match std::fs::read_to_string(&abs_file) {
            Ok(source) => source,
            Err(error) => {
                info.unresolved_items += 1;
                info.warnings
                    .push(format!("semantic read failed for {}: {error}", abs_file.display()));
                continue;
            }
        };

        let file_uri = match path_to_uri(&abs_file) {
            Ok(uri) => uri,
            Err(error) => {
                info.unresolved_items += 1;
                info.warnings.push(format!(
                    "semantic URI conversion failed for {}: {error:#}",
                    abs_file.display()
                ));
                continue;
            }
        };

        if !opened.contains(&rel_file) {
            if let Err(error) = did_open(&mut server, &file_uri, &source) {
                info.unresolved_items += 1;
                info.warnings.push(format!(
                    "semantic didOpen failed for {}: {error:#}",
                    abs_file.display()
                ));
                continue;
            }
            opened.insert(rel_file.clone());
        }

        if let Err(error) = maybe_enrich_hover(
            graph,
            node_index,
            &source,
            &file_uri,
            range.start_line,
            &mut server,
            &mut ids,
            &mut info,
        ) {
            info.warnings.push(format!(
                "semantic hover failed for {}:{}: {error:#}",
                rel_file, range.start_line
            ));
        }

        if let Err(error) = maybe_enrich_references(
            graph,
            node_index,
            &source,
            &file_uri,
            range.start_line,
            &root,
            &mut server,
            &mut ids,
            &mut info,
        ) {
            info.warnings.push(format!(
                "semantic references failed for {}:{}: {error:#}",
                rel_file, range.start_line
            ));
        }
    }

    if let Err(error) = shutdown(&mut server, &mut ids) {
        info.warnings.push(format!("semantic shutdown failed: {error:#}"));
    }

    Ok(info)
}

fn spawn_server(path: &Path) -> Result<Child> {
    let mut command = Command::new(path);
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("pyright-langserver"))
    {
        command.arg("--stdio");
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn language server")
}

fn initialize(server: &mut Child, root: &Path) -> Result<()> {
    let root_uri = path_to_uri(root)?;
    let params = json!({
        "processId": std::process::id(),
        "clientInfo": { "name": "pyscope", "version": env!("CARGO_PKG_VERSION") },
        "rootUri": root_uri,
        "workspaceFolders": [{
            "uri": root_uri,
            "name": root.file_name().and_then(|name| name.to_str()).unwrap_or("project")
        }],
        "capabilities": {
            "textDocument": {
                "hover": { "contentFormat": ["markdown", "plaintext"] },
                "references": {}
            }
        },
        "initializationOptions": serde_json::Value::Null
    });

    let _ = send_request(server, 1, "initialize", params)?;
    send_notification(server, "initialized", json!({}))
}

fn shutdown(server: &mut Child, next_id: &mut u64) -> Result<()> {
    let _ = send_request(server, *next_id, "shutdown", Value::Null)?;
    *next_id += 1;
    let _ = send_notification(server, "exit", Value::Null);
    let _ = server.wait();
    Ok(())
}

fn did_open(server: &mut Child, uri: &Url, source: &str) -> Result<()> {
    send_notification(
        server,
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "python",
                "version": 1,
                "text": source
            }
        }),
    )
}

fn maybe_enrich_hover(
    graph: &mut CodeGraph,
    node_index: usize,
    source: &str,
    file_uri: &Url,
    line: usize,
    server: &mut Child,
    next_id: &mut u64,
    info: &mut SemanticInfo,
) -> Result<()> {
    let params = json!({
        "textDocument": { "uri": file_uri },
        "position": lsp_position(source, line)
    });
    let value = send_request(server, *next_id, "textDocument/hover", params)?;
    *next_id += 1;

    if value.is_null() {
        return Ok(());
    }

    let hover: Hover = serde_json::from_value(value)?;
    if let Some(ty) = hover_type(&hover) {
        info.confirmed_symbols += 1;
        let node = graph
            .nodes
            .get_mut(node_index)
            .context("semantic hover target index out of bounds")?;
        let already_has_type = node.signature.as_deref().is_some_and(|sig| sig.contains(&ty));
        if !already_has_type {
            let updated = match node.signature.take() {
                Some(existing) if !existing.trim().is_empty() => {
                    format!("{} -> {}", existing.trim(), ty.trim())
                }
                _ => ty.trim().to_string(),
            };
            node.signature = Some(updated);
            info.enriched_symbols += 1;
        }
    }

    Ok(())
}

fn maybe_enrich_references(
    graph: &mut CodeGraph,
    node_index: usize,
    source: &str,
    file_uri: &Url,
    line: usize,
    root: &Path,
    server: &mut Child,
    next_id: &mut u64,
    info: &mut SemanticInfo,
) -> Result<()> {
    let params = json!({
        "textDocument": { "uri": file_uri },
        "position": lsp_position(source, line),
        "context": ReferenceContext { include_declaration: false }
    });
    let value = send_request(server, *next_id, "textDocument/references", params)?;
    *next_id += 1;

    if value.is_null() {
        return Ok(());
    }

    let refs: Vec<lsp_types::Location> = serde_json::from_value(value)?;
    let target_id = graph
        .nodes
        .get(node_index)
        .map(|node| node.id.clone())
        .context("semantic reference target index out of bounds")?;

    let file_index = build_file_function_index(graph);
    for location in refs {
        let Some(path) = location.uri.to_file_path().ok() else {
            info.unresolved_items += 1;
            continue;
        };
        let Some(rel_file) = relative_path(root, &path) else {
            info.unresolved_items += 1;
            continue;
        };

        let ref_line = location.range.start.line as usize + 1;
        let Some(caller_id) = resolve_caller_id(&file_index, &rel_file, ref_line) else {
            info.unresolved_items += 1;
            continue;
        };

        if caller_id == target_id || has_calls_edge(graph, &caller_id, &target_id, ref_line) {
            continue;
        }

        graph.edges.push(Edge {
            from: caller_id,
            to: target_id.clone(),
            kind: EdgeKind::Calls,
            label: None,
            evidence: Some(Location {
                file: rel_file.clone(),
                line: ref_line,
            }),
            weight: 1,
            source: EdgeSource::Inferred,
            certainty: EdgeCertainty::Possible,
            call_style: Some("lsp_reference".to_string()),
        });
        info.enriched_edges += 1;
        info.confirmed_edges += 1;
    }

    Ok(())
}

struct FunctionSite {
    id: String,
    start_line: usize,
    end_line: usize,
}

fn build_file_function_index(graph: &CodeGraph) -> HashMap<String, Vec<FunctionSite>> {
    let function_kinds = [
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::AsyncFunction,
        NodeKind::AsyncMethod,
        NodeKind::ClassMethod,
        NodeKind::StaticMethod,
        NodeKind::Property,
    ];

    let mut by_file: HashMap<String, Vec<FunctionSite>> = HashMap::new();
    for node in &graph.nodes {
        if !function_kinds.contains(&node.kind) {
            continue;
        }
        let Some(file) = &node.file else {
            continue;
        };
        let Some(range) = &node.range else {
            continue;
        };
        by_file.entry(file.clone()).or_default().push(FunctionSite {
            id: node.id.clone(),
            start_line: range.start_line,
            end_line: range.end_line,
        });
    }
    by_file
}

fn resolve_caller_id(
    file_index: &HashMap<String, Vec<FunctionSite>>,
    rel_file: &str,
    line: usize,
) -> Option<String> {
    let mut best: Option<&FunctionSite> = None;
    for node in file_index.get(rel_file)? {
        if node.start_line <= line && line <= node.end_line {
            match best {
                Some(current) => {
                    let current_span = current.end_line.saturating_sub(current.start_line);
                    let new_span = node.end_line.saturating_sub(node.start_line);
                    if new_span < current_span {
                        best = Some(node);
                    }
                }
                None => best = Some(node),
            }
        }
    }
    best.map(|node| node.id.clone())
}

fn has_calls_edge(graph: &CodeGraph, from: &str, to: &str, line: usize) -> bool {
    graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Calls
            && edge.from == from
            && edge.to == to
            && edge.evidence.as_ref().map(|loc| loc.line) == Some(line)
    })
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().to_string())
}

fn send_request(server: &mut Child, id: u64, method: &str, params: Value) -> Result<Value> {
    write_message(
        server,
        &json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }),
    )?;

    loop {
        let (response_id, payload) = read_response(server)?;
        if response_id != id {
            continue;
        }
        if let Some(error) = payload.get("error") {
            anyhow::bail!("LSP {method} failed: {}", error);
        }
        return Ok(payload.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn send_notification(server: &mut Child, method: &str, params: Value) -> Result<()> {
    write_message(
        server,
        &json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }),
    )
}

fn read_response(server: &mut Child) -> Result<(u64, Value)> {
    loop {
        let value = read_message(server)?;
        let Some(id) = value.get("id").and_then(Value::as_u64) else {
            continue;
        };
        return Ok((id, value));
    }
}

fn write_message(server: &mut Child, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let stdin = server.stdin.as_mut().context("LSP stdin unavailable")?;
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len())?;
    stdin.write_all(&body)?;
    stdin.flush()?;
    Ok(())
}

fn read_message(server: &mut Child) -> Result<Value> {
    let stdout = server.stdout.as_mut().context("LSP stdout unavailable")?;
    let mut header = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        stdout.read_exact(&mut byte)?;
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let header_text = String::from_utf8(header)?;
    let length = header_text
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-length:")
                .map(|rest| rest.trim().parse::<usize>())
        })
        .transpose()?
        .context("missing Content-Length header")?;

    let mut body = vec![0_u8; length];
    stdout.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
