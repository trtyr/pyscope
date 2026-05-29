# Implementation Status: Initial Build

## Current Phase

Semantic enrichment pass for indexed Python graphs.

## Active TODO

- Add `src/semantic/` with Python LSP JSON-RPC client
- Persist semantic metadata on `CodeGraph`
- Wire index command to run semantic enrichment by default
- Verify semantic path with `cargo check`

## Blockers

- None yet; enrichment must degrade gracefully when no language server is installed

## Next Commit Target

- Semantic module compiles and index command stores `graph.semantic`

## Last Verification

- Pre-change: `cargo run -- index .` succeeds for AST-only graph generation
- Support change: `src/model.rs` expanded `NodeKind`/`EdgeKind`; `cargo check` passes
- Analyzer follow-up: `src/analyzer/visitors.rs` now detects expanded class/function/variable kinds and new decorator/import/await/override edges; `cargo check && cargo build` passes
- Analyze follow-up: added `analyze hotspots`, `analyze diff`, and `analyze refactor-order` with new `src/analyze/{hotspots,diff,refactor_order}.rs` modules; `cargo check` passes and `crabmap index .` refreshed the Rust graph
- Nav expansion: new `nav guide|entries|clusters|quality` commands added; `cargo check && cargo test` pass
- Rust code map refreshed with `crabmap index .`
- Analyze expansion: new `analyze type-coverage`, `analyze async-map`, and `analyze decorator-usage` commands added; `cargo check` passes
- Query symbols filter expansion: added `--dynamic`, `--legacy`, `--async-only`, and `--decorator`; `cargo check && cargo build` passes and `crabmap index .` refreshed the Rust graph
- Query expansion: new `query risk` command added with impact summary, test-coverage discovery, risk scoring, and suggested follow-up commands; `cargo check && cargo build` pass; `crabmap index .` refreshed the Rust graph
- CI integration: added `ci check` command with configurable health/cycle/god-module/dead-code thresholds; `cargo check && cargo build` pass; smoke tests confirm pass JSON and exit-code-1 failure path; `crabmap index .` refreshed the Rust graph
