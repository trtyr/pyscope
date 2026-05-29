# Storage and State

## Graph File

- Path: `<project>/.pyscope/pyscope.json.gz`
- Format: gzip-compressed JSON
- Schema: same as crabmap's CodeGraph (nodes, edges, project metadata)
- Load: `store::load(path)` → `CodeGraph`
- Save: `store::save(path, &CodeGraph)`

## Schema

```rust
CodeGraph {
    schema_version: u32,     // 1
    project: Project,        // name, root, files, packages
    nodes: Vec<Node>,        // all discovered symbols
    edges: Vec<Edge>,        // all relationships
    warnings: Vec<String>,   // parse errors, unknown constructs
    generated_at_ms: u128,   // timestamp
}
```

## Auto-discovery

When querying without explicit `--graph`:
1. Check `./.pyscope/pyscope.json.gz`
2. If not found, scan `.pyscope/` for any `*.json.gz`
3. If multiple found, load all and merge graphs

This enables multi-project queries (e.g., analyzing a monorepo with multiple
Python packages).
