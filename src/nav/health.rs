use crate::model::{CodeGraph, EdgeKind, EdgeSource, EdgeCertainty, NodeKind};
use crate::query::index::QueryIndex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

/// Strip `src/` prefix from file path, then take the first path component.
fn module_path(file: &str) -> &str {
    let stripped = file.strip_prefix("src/").unwrap_or(file);
    stripped.split('/').next().unwrap_or(stripped)
}

/// Check if a file path looks like a test file.
fn is_test_file(file: &str) -> bool {
    file.contains("/test") || file.contains("/tests/") || file.contains("_test.")
}

/// Truncate to max chars with `...` suffix.
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



/// Computes a 0-100 architecture health score.
pub fn health(graph: &CodeGraph, limit: usize) -> Value {
    let index = QueryIndex::new(graph);

    // 1. Dead public symbols: pub symbols with zero incoming edges, exclude test files
    let mut dead_public: Vec<Value> = Vec::new();
    for node in &graph.nodes {
        let vis = node.visibility.as_deref().unwrap_or("");
        if vis != "pub" && vis != "public" {
            continue;
        }
        let file = node.file.as_deref().unwrap_or("");
        if is_test_file(file) {
            continue;
        }
        let incoming = index.edges(&node.id, false);
        if incoming.is_empty() {
            dead_public.push(json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
                "file": node.file,
                "range": node.range,
            }));
        }
    }
    dead_public.truncate(limit);

    // 2. God modules: files with >= 40 meaningful symbols
    let mut file_symbol_count: HashMap<&str, usize> = HashMap::new();
    for node in &graph.nodes {
        match node.kind {
            NodeKind::Function
            | NodeKind::Method
            | NodeKind::AsyncFunction
            | NodeKind::AsyncMethod
            | NodeKind::Class => {
                if let Some(file) = &node.file {
                    *file_symbol_count.entry(file.as_str()).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }
    let mut god_modules: Vec<Value> = file_symbol_count
        .iter()
        .filter(|(_, count)| **count >= 40)
        .map(|(&file, &count)| {
            json!({
                "file": file,
                "symbol_count": count,
            })
        })
        .collect();
    god_modules.sort_by(|a, b| {
        b["symbol_count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["symbol_count"].as_u64().unwrap_or(0))
    });
    god_modules.truncate(limit);

    // 3. Cycles: module dependency graph from Calls/Imports with high-confidence edges
    let mut module_deps: HashMap<String, HashSet<String>> = HashMap::new();
    let mut all_modules: HashSet<String> = HashSet::new();

    for edge in &graph.edges {
        if edge.source != EdgeSource::Ast || edge.certainty != EdgeCertainty::Definite {
            continue;
        }
        if edge.kind != EdgeKind::Calls && edge.kind != EdgeKind::Imports {
            continue;
        }
        let from_node = match index.node(&edge.from) {
            Some(n) => n,
            None => continue,
        };
        let to_node = match index.node(&edge.to) {
            Some(n) => n,
            None => continue,
        };
        let from_file = match &from_node.file {
            Some(f) => f,
            None => continue,
        };
        let to_file = match &to_node.file {
            Some(f) => f,
            None => continue,
        };
        let from_mod = module_path(from_file).to_string();
        let to_mod = module_path(to_file).to_string();
        if from_mod == to_mod {
            continue;
        }
        all_modules.insert(from_mod.clone());
        all_modules.insert(to_mod.clone());
        module_deps
            .entry(from_mod)
            .or_default()
            .insert(to_mod);
    }

    // BFS cycle detection up to depth 6
    let mut cycles: Vec<Value> = Vec::new();
    let mut checked: HashSet<String> = HashSet::new();

    for start in &all_modules {
        if checked.contains(start) {
            continue;
        }
        // BFS from start, looking for a path back to start
        let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
        let mut seen: HashSet<String> = HashSet::new();
        queue.push_back((start.clone(), vec![start.clone()]));
        seen.insert(start.clone());

        while let Some((current, path)) = queue.pop_front() {
            if path.len() > 6 {
                continue;
            }
            if let Some(deps) = module_deps.get(&current) {
                for dep in deps {
                    if dep == start && path.len() > 1 {
                        let mut cycle_path = path.clone();
                        cycle_path.push(dep.clone());
                        cycles.push(json!({
                            "path": cycle_path,
                            "length": cycle_path.len(),
                        }));
                        checked.insert(start.clone());
                        break;
                    }
                    if !seen.contains(dep) {
                        seen.insert(dep.clone());
                        let mut new_path = path.clone();
                        new_path.push(dep.clone());
                        queue.push_back((dep.clone(), new_path));
                    }
                }
            }
        }
        checked.insert(start.clone());
    }
    cycles.sort_by(|a, b| {
        b["length"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["length"].as_u64().unwrap_or(0))
    });
    cycles.truncate(limit);

    // 4. Hot symbols: total degree >= 20
    let mut hot_symbols: Vec<Value> = Vec::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::File || node.kind == NodeKind::Project {
            continue;
        }
        let deg = index.degree(&node.id);
        if deg >= 20 {
            hot_symbols.push(json!({
                "id": node.id,
                "kind": node.kind.as_str(),
                "qualified_name": node.qualified_name,
                "file": node.file,
                "range": node.range,
                "degree": deg,
                "signature": node.signature.as_deref().map(|s| truncate(s, 120)),
                "docs": node.docs.as_deref().and_then(|d| d.lines().next()).map(|l| truncate(l, 120)),
            }));
        }
    }
    hot_symbols.sort_by(|a, b| {
        b["degree"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["degree"].as_u64().unwrap_or(0))
    });
    hot_symbols.truncate(limit);

    // Score formula: max(0, 100 - cycles*5 - god_modules*3 - dead*1 - warnings*2)
    // Each category capped at 10 items for scoring
    let cycle_count = cycles.len().min(10);
    let god_count = god_modules.len().min(10);
    let dead_count = dead_public.len().min(10);
    let warning_count = graph.warnings.len().min(10);

    let score_raw: i64 = 100
        - (cycle_count as i64 * 5)
        - (god_count as i64 * 3)
        - (dead_count as i64 * 1)
        - (warning_count as i64 * 2);
    let score = score_raw.max(0) as usize;

    let label = match score {
        80..=100 => "high",
        60..=79 => "medium",
        40..=59 => "low",
        _ => "critical",
    };

    json!({
        "kind": "health",
        "score": score,
        "label": label,
        "cycles": cycles,
        "god_modules": god_modules,
        "dead_public_symbols": dead_public,
        "hot_symbols": hot_symbols,
        "warnings": graph.warnings,
    })
}
