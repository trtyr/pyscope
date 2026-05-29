use crate::analyzer::builder::Builder;
use crate::analyzer::visitors;
use crate::model::*;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Index a Python project and return the code graph.
pub fn index_project(project: &Path, include_tests: bool) -> Result<CodeGraph> {
    let root = project
        .canonicalize()
        .context("Failed to resolve project path")?;
    let root_str = root.display().to_string();

    let mut builder = Builder::new();

    // Create the project root node
    builder.add_node(
        NodeKind::Project,
        &root_str,
        &root_str,
        None,
        1,
        1,
        None,
        None,
        None,
    );

    // Discover and index .py files
    let py_files = discover_python_files(&root, include_tests);
    let _file_count = py_files.len();

    for file_path in &py_files {
        if let Err(e) = index_file(&mut builder, file_path, &root) {
            builder
                .warnings
                .push(format!("Failed to index {}: {}", file_path.display(), e));
        }
    }

    // Resolve all pending call/import/inheritance edges
    builder.resolve_pending();

    let generated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    Ok(CodeGraph {
        schema_version: 1,
        project: Project {
            root: root_str,
            packages: vec![Package {
                name: detect_package_name(&root),
                root: root.display().to_string(),
                files: py_files.iter().map(|p| p.display().to_string()).collect(),
            }],
        },
        nodes: builder.nodes,
        edges: builder.edges,
        semantic: None,
        warnings: builder.warnings,
        generated_at_ms,
    })
}

/// Index a single Python file.
fn index_file(builder: &mut Builder, file_path: &Path, project_root: &Path) -> Result<()> {
    let source = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    visitors::visit_file(builder, &source, file_path, project_root)
        .with_context(|| format!("Failed to visit {}", file_path.display()))
}

/// Discover all .py files in a project directory.
fn discover_python_files(root: &Path, include_tests: bool) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "py"))
        .filter(|e| {
            if include_tests {
                return true;
            }
            is_not_test_file(e.path())
        })
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

fn is_not_test_file(path: &Path) -> bool {
    let path_str = path.display().to_string();
    if path_str.contains("/test/") || path_str.contains("/tests/") {
        return false;
    }
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with("test_") || name.ends_with("_test.py") {
            return false;
        }
    }
    true
}

fn detect_package_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
