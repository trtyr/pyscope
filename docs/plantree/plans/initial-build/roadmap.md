# Roadmap: Initial Build

## Done

- Project scaffold: Cargo.toml, src/main.rs skeleton
- Architecture plan (baseline/module-map.md)
- AI/LLM integration: `src/config.rs`, `src/llm.rs`, `src/rag/*`, `nav ask`, `nav retrieve`, and `config` subcommands
- Verification gate: `cargo check` passes after AI/LLM integration

## In Progress

- Semantic enrichment via Python LSP (`src/semantic/`, index-time enrichment pass)

## Next

### Phase 1: Core Model + CLI

**Goal**: Empty graph that compiles and parses CLI args.

- [ ] `src/model.rs`: CodeGraph, Node, Edge, NodeKind, EdgeKind, Range, Location
- [ ] `src/cli.rs`: clap definitions: index, query (inspect/trace/find/scope)
- [ ] `src/store.rs`: gzip JSON load/save
- [ ] `src/main.rs`: CLI dispatch skeleton (no real logic yet)
- [ ] `Cargo.toml`: add dependencies (clap, serde, serde_json, anyhow, flate2)
- [ ] Gate: `cargo check` passes

### Phase 2: Python Parser Integration

**Goal**: Parse .py files and extract basic symbols.

- [ ] `src/analyzer/helpers.rs`: file listing, doc extraction, metrics
- [ ] `src/analyzer/builder.rs`: node/edge builder
- [ ] `src/analyzer/visitors.rs`: AST walker (functions, classes, imports)
- [ ] `src/analyzer/index.rs`: main indexing pipeline
- [ ] `Cargo.toml`: add rustpython-parser
- [ ] Gate: `cargo run -- index tests/fixtures/sample/` produces valid graph JSON

### Phase 3: Query Infrastructure

**Goal**: Load graph and answer basic queries.

- [ ] `src/query/index.rs`: QueryIndex (adjacency index)
- [ ] `src/query/find.rs`: node resolution + fuzzy matching
- [ ] `src/query/traversal.rs`: graph traversal (walk, path)
- [ ] `src/query/commands.rs`: inspect, trace, find, scope
- [ ] Gate: `cargo run -- query inspect func_name` returns valid JSON

### Phase 4: Call Graph + Import Resolution

**Goal**: Build actual call graph with cross-file import resolution.

- [ ] Call resolution: same-module first, then imports
- [ ] Import resolution: `import foo` → find `foo.py`
- [ ] Class method resolution: `self.method()` → find method in class
- [ ] Decorator capture
- [ ] Gate: query callees/callers on fixture project

### Phase 5: Test Fixtures + Integration Tests

**Goal**: Reproducible test suite.

- [ ] `tests/fixtures/sample/`: minimal Python project
- [ ] `tests/integration.rs`: basic index/query tests
- [ ] Gate: `cargo test` passes

### Phase 6: Real Project Validation

**Goal**: Index and query a real 100+ file Python project.

- [ ] Index ppt-master (100+ .py files)
- [ ] Verify: query inspect/trace/find return valid results
- [ ] Measure: indexing time, graph size, query latency

## Deferred

- Web viewer (copy crabmap's embedded viewer later)
- MIR lowering (Python bytecode analysis)
- Incremental indexing
- Cross-package dependency tracking
