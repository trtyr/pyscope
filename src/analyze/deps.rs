use crate::model::{CodeGraph, EdgeKind};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Drop the filename and extension, keep only the directory path.
/// e.g. "src/analyzer/index.rs" -> "src/analyzer"
/// e.g. "skills/ppt-master/scripts/config.py" -> "skills/ppt-master/scripts"
/// e.g. "foo.py" -> "foo.py"
fn cluster_key(file: &str) -> String {
    let components: Vec<&str> = file.split('/').collect();
    if components.len() >= 2 {
        components[..components.len() - 1].join("/")
    } else {
        file.to_string()
    }
}

/// BFS that greedily picks the longest path at each level.
/// Returns the chain of module keys visited.
fn bfs_chain(
    adj: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
    max_depth: usize,
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    visited.insert(start.to_string());
    let mut chain = vec![start.to_string()];
    let mut current = start.to_string();

    for _ in 0..max_depth {
        let neighbors = adj.get(&current).cloned().unwrap_or_default();
        let next = neighbors
            .iter()
            .filter(|n| !visited.contains(*n))
            .next()
            .cloned();
        match next {
            Some(node) => {
                visited.insert(node.clone());
                chain.push(node.clone());
                current = node;
            }
            None => break,
        }
    }
    chain
}

pub fn deps(graph: &CodeGraph, from: Option<&str>, limit: usize) -> Value {
    // 1. Build file-level dependency graph
    let relevant_kinds = [
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::UsesType,
        EdgeKind::Declares,
    ];

    let mut weight_map: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut all_clusters = BTreeSet::new();

    // Build node_id -> file lookup
    let node_file: BTreeMap<&str, &str> = graph
        .nodes
        .iter()
        .filter_map(|n| n.file.as_deref().map(|f| (n.id.as_str(), f)))
        .collect();

    for edge in &graph.edges {
        if !relevant_kinds.contains(&edge.kind) {
            continue;
        }
        let src_file = match node_file.get(edge.from.as_str()) {
            Some(f) => f,
            None => continue,
        };
        let dst_file = match node_file.get(edge.to.as_str()) {
            Some(f) => f,
            None => continue,
        };
        let src_cluster = cluster_key(src_file);
        let dst_cluster = cluster_key(dst_file);
        if src_cluster == dst_cluster {
            continue;
        }
        all_clusters.insert(src_cluster.clone());
        all_clusters.insert(dst_cluster.clone());
        *weight_map.entry((src_cluster, dst_cluster)).or_insert(0) += 1;
    }

    // 2. Filter by `from` if specified
    let filtered: Vec<((String, String), usize)> = if let Some(from_module) = from {
        let from_key = cluster_key(from_module);
        weight_map
            .into_iter()
            .filter(|((f, _), _)| f == &from_key)
            .collect()
    } else {
        weight_map.into_iter().collect()
    };

    // 3. Sort by weight desc, take top `limit`
    let mut items: Vec<Value> = filtered
        .into_iter()
        .map(|((from, to), weight)| json!({ "from": from, "to": to, "weight": weight }))
        .collect();
    items.sort_by(|a, b| {
        b["weight"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["weight"].as_u64().unwrap_or(0))
    });
    items.truncate(limit);

    // 4. Build adjacency for recompile impact analysis
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for cluster in &all_clusters {
        adj.entry(cluster.clone()).or_default();
    }
    // Rebuild weight_map for adjacency (use all edges, not just filtered)
    let mut full_weight: BTreeMap<(String, String), usize> = BTreeMap::new();
    for edge in &graph.edges {
        if !relevant_kinds.contains(&edge.kind) {
            continue;
        }
        let src_file = match node_file.get(edge.from.as_str()) {
            Some(f) => f,
            None => continue,
        };
        let dst_file = match node_file.get(edge.to.as_str()) {
            Some(f) => f,
            None => continue,
        };
        let src_cluster = cluster_key(src_file);
        let dst_cluster = cluster_key(dst_file);
        if src_cluster == dst_cluster {
            continue;
        }
        *full_weight
            .entry((src_cluster.clone(), dst_cluster.clone()))
            .or_insert(0) += 1;
        adj.entry(src_cluster).or_default().insert(dst_cluster);
    }

    // recompile_impact: module pairs where BFS chain length >= 3
    let mut recompile_impact: Vec<Value> = Vec::new();
    for cluster in &all_clusters {
        let chain = bfs_chain(&adj, cluster, 10);
        if chain.len() >= 3 {
            recompile_impact.push(json!({
                "from": chain.first().unwrap(),
                "to": chain.last().unwrap(),
                "chain_length": chain.len(),
                "chain": chain,
            }));
        }
    }
    recompile_impact.sort_by(|a, b| {
        b["chain_length"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["chain_length"].as_u64().unwrap_or(0))
    });

    // 5. total_recompile_modules: BFS transitive closure from the most-connected module
    let most_connected = adj
        .iter()
        .max_by_key(|(_, neighbors)| neighbors.len())
        .map(|(k, _)| k.clone());

    let total_recompile_modules = if let Some(start) = &most_connected {
        let mut visited = BTreeSet::new();
        visited.insert(start.clone());
        let mut queue = VecDeque::new();
        queue.push_back(start.clone());
        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&current) {
                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
        visited.len()
    } else {
        0
    };

    json!({
        "kind": "deps",
        "items": items,
        "recompile_impact": recompile_impact,
        "total_recompile_modules": total_recompile_modules,
    })
}
