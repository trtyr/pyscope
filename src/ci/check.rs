use crate::model::CodeGraph;
use anyhow::Result;
use serde_json::{Value, json};

pub fn check(
    graph: &CodeGraph,
    min_health: u8,
    max_cycles: usize,
    max_god_modules: usize,
    max_dead_code: usize,
) -> Result<Value> {
    let health = crate::nav::health(graph, usize::MAX);

    let health_score = health["score"].as_u64().unwrap_or(0) as usize;
    let cycle_count = health["cycles"].as_array().map_or(0, Vec::len);
    let god_module_count = health["god_modules"].as_array().map_or(0, Vec::len);
    let dead_code_count = health["dead_public_symbols"].as_array().map_or(0, Vec::len);

    let checks = vec![
        threshold_check(
            "min_health",
            health_score,
            min_health as usize,
            health_score >= min_health as usize,
        ),
        threshold_check(
            "max_cycles",
            cycle_count,
            max_cycles,
            cycle_count <= max_cycles,
        ),
        threshold_check(
            "max_god_modules",
            god_module_count,
            max_god_modules,
            god_module_count <= max_god_modules,
        ),
        threshold_check(
            "max_dead_code",
            dead_code_count,
            max_dead_code,
            dead_code_count <= max_dead_code,
        ),
    ];

    let passed = checks
        .iter()
        .all(|check| check["passed"].as_bool().unwrap_or(false));

    Ok(json!({
        "kind": "ci_check",
        "passed": passed,
        "checks": checks,
        "summary": {
            "passed": checks.iter().filter(|check| check["passed"].as_bool().unwrap_or(false)).count(),
            "failed": checks.iter().filter(|check| !check["passed"].as_bool().unwrap_or(false)).count(),
            "total": checks.len(),
            "health_label": health["label"],
        }
    }))
}

fn threshold_check(name: &str, actual: usize, threshold: usize, passed: bool) -> Value {
    json!({
        "name": name,
        "passed": passed,
        "actual": actual,
        "threshold": threshold,
    })
}
