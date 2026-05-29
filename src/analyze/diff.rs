use crate::model::CodeGraph;
use crate::query::index::QueryIndex;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn resolve_project_root(graph: &CodeGraph, project_root: Option<&Path>) -> PathBuf {
    project_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&graph.project.root))
}

fn normalize_path(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn file_matches(node_file: Option<&str>, changed_files: &BTreeSet<String>) -> bool {
    let Some(file) = node_file else {
        return false;
    };
    changed_files.contains(file)
        || changed_files
            .iter()
            .any(|changed| file.ends_with(changed) || changed.ends_with(file))
}

fn warning_result(base: &str, warning: String) -> Value {
    json!({
        "kind": "diff",
        "added_nodes": [],
        "removed_nodes": [],
        "added_edges": [],
        "removed_edges": [],
        "changed_files": [],
        "summary": {
            "base": base,
            "added_nodes": 0,
            "removed_nodes": 0,
            "added_edges": 0,
            "removed_edges": 0,
        },
        "warning": warning,
    })
}

pub fn diff(graph: &CodeGraph, base: &str, project_root: Option<&Path>) -> Value {
    let root = resolve_project_root(graph, project_root);
    let output = match Command::new("git")
        .current_dir(&root)
        .arg("diff")
        .arg("--name-only")
        .arg(base)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return warning_result(base, format!("failed to execute git diff: {err}"));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let warning = if stderr.is_empty() {
            "git diff failed".to_string()
        } else {
            format!("git diff failed: {stderr}")
        };
        return warning_result(base, warning);
    }

    let changed_files: BTreeSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| normalize_path(&root.join(line), &root))
        .collect();

    let changed_file_list: Vec<String> = changed_files.iter().cloned().collect();
    let affected_node_ids: BTreeSet<String> = graph
        .nodes
        .iter()
        .filter(|node| file_matches(node.file.as_deref(), &changed_files))
        .map(|node| node.id.clone())
        .collect();

    let index = QueryIndex::new(graph);
    let added_nodes: Vec<Value> = affected_node_ids
        .iter()
        .filter_map(|id| index.node(id))
        .map(|node| {
            json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
            })
        })
        .collect();

    let added_edges: Vec<Value> = graph
        .edges
        .iter()
        .filter(|edge| {
            affected_node_ids.contains(&edge.from) || affected_node_ids.contains(&edge.to)
        })
        .map(|edge| {
            json!({
                "from": edge.from,
                "to": edge.to,
                "kind": edge.kind.as_str(),
            })
        })
        .collect();

    json!({
        "kind": "diff",
        "added_nodes": added_nodes,
        "removed_nodes": [],
        "added_edges": added_edges,
        "removed_edges": [],
        "changed_files": changed_file_list,
        "summary": {
            "base": base,
            "added_nodes": affected_node_ids.len(),
            "removed_nodes": 0,
            "added_edges": added_edges.len(),
            "removed_edges": 0,
        },
    })
}
