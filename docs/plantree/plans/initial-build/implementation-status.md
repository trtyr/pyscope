# Implementation Status: Initial Build

## Current Phase

Semantic enrichment pass for indexed Python graphs.

## Active TODO

- Add `src/semantic/` with Python LSP JSON-RPC client
- Persist semantic metadata on `CodeGraph`
- Wire index command to run semantic enrichment by default
- Verify with `cargo check`

## Blockers

- None yet; enrichment must degrade gracefully when no language server is installed

## Next Commit Target

- Semantic module compiles and index command stores `graph.semantic`

## Last Verification

- Pre-change: `cargo run -- index .` succeeds for AST-only graph generation
