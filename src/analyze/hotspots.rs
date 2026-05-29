use crate::model::CodeGraph;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Default)]
struct FileStats {
    commits: usize,
    authors: BTreeSet<String>,
    last_commit: Option<String>,
}

fn resolve_project_root(graph: &CodeGraph, project_root: Option<&Path>) -> PathBuf {
    project_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&graph.project.root))
}

fn normalize_path(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn warning_result(period: String, warning: String) -> Value {
    json!({
        "kind": "hotspots",
        "hotspots": [],
        "cochange": [],
        "summary": {
            "total_files": 0,
            "total_commits": 0,
            "period": period,
        },
        "warning": warning,
    })
}

pub fn hotspots(
    graph: &CodeGraph,
    limit: usize,
    since: Option<&str>,
    project_root: Option<&Path>,
) -> Value {
    let root = resolve_project_root(graph, project_root);
    let period = since.unwrap_or("all time").to_string();

    let mut cmd = Command::new("git");
    cmd.current_dir(&root)
        .arg("log")
        .arg("--format=__COMMIT__%n%H%n%aN%n%cs")
        .arg("--name-only");
    if let Some(since_value) = since {
        cmd.arg(format!("--since={since_value}"));
    }

    let output = match cmd.output() {
        Ok(output) => output,
        Err(err) => {
            return warning_result(period, format!("failed to execute git log: {err}"));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let warning = if stderr.is_empty() {
            "git log failed".to_string()
        } else {
            format!("git log failed: {stderr}")
        };
        return warning_result(period, warning);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files: BTreeMap<String, FileStats> = BTreeMap::new();
    let mut cochange: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut commit_count = 0usize;

    for block in stdout
        .split("__COMMIT__\n")
        .filter(|block| !block.trim().is_empty())
    {
        let mut lines = block.lines();
        let Some(_hash) = lines.next() else {
            continue;
        };
        let author = lines.next().unwrap_or_default().trim().to_string();
        let date = lines.next().unwrap_or_default().trim().to_string();

        let mut commit_files = BTreeSet::new();
        for raw_path in lines {
            let trimmed = raw_path.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = normalize_path(&root.join(trimmed), &root);
            if normalized.is_empty() {
                continue;
            }

            let entry = files.entry(normalized.clone()).or_default();
            entry.commits += 1;
            if !author.is_empty() {
                entry.authors.insert(author.clone());
            }
            let should_replace = match entry.last_commit.as_ref() {
                Some(current) => date > *current,
                None => true,
            };
            if should_replace {
                entry.last_commit = Some(date.clone());
            }
            commit_files.insert(normalized);
        }

        if commit_files.is_empty() {
            continue;
        }

        commit_count += 1;
        let commit_files: Vec<String> = commit_files.into_iter().collect();
        for i in 0..commit_files.len() {
            for j in (i + 1)..commit_files.len() {
                *cochange
                    .entry((commit_files[i].clone(), commit_files[j].clone()))
                    .or_insert(0) += 1;
            }
        }
    }

    let mut hotspots: Vec<Value> = files
        .iter()
        .map(|(file, stats)| {
            json!({
                "file": file,
                "commits": stats.commits,
                "authors": stats.authors.len(),
                "last_commit": stats.last_commit,
            })
        })
        .collect();
    hotspots.sort_by(|a, b| {
        b["commits"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["commits"].as_u64().unwrap_or(0))
            .then_with(|| {
                a["file"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["file"].as_str().unwrap_or(""))
            })
    });
    hotspots.truncate(limit);

    let mut cochange_items: Vec<Value> = cochange
        .into_iter()
        .map(|((left, right), count)| {
            json!({
                "files": [left, right],
                "count": count,
            })
        })
        .collect();
    cochange_items.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    cochange_items.truncate(limit);

    json!({
        "kind": "hotspots",
        "hotspots": hotspots,
        "cochange": cochange_items,
        "summary": {
            "total_files": files.len(),
            "total_commits": commit_count,
            "period": period,
        },
    })
}
