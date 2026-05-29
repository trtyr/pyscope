[![Rust](https://img.shields.io/badge/rust-1.85+-ed8225?style=flat-square&logo=rust&logoColor=white)](https://rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-22C55E?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-cross--platform-8B5CF6?style=flat-square)]()

**Python code satellite map.** pyscope indexes a Python project into a structured call graph, then search, trace, navigate, and analyze — without reading `.py` files one by one. Built on `rustpython-parser` and the crabmap graph engine. Designed for AI agents and human developers alike.

[🔧 Quick Start](#quick-start) · [📋 Commands](#commands) · [🏗️ Architecture](#architecture) · [📊 Concepts](#concepts)

---

## Quick Start

```bash
# Build and install
cargo build --release
cargo install --path .

# Index a Python project
pyscope index /path/to/project
# => indexed 2864 nodes, 3941 edges in 107 files

# Get the big picture (~8k tokens for AI context)
pyscope nav map

# Search for symbols
pyscope query find "handler"

# Inspect a function — source code included by default
pyscope query inspect myproject.handlers.process_data

# Trace call chains
pyscope query trace myproject.handlers.process_data --direction both --depth 3

# See what breaks if you change a function
pyscope query impact myproject.handlers.process_data

# Check which tests to run
pyscope analyze tests myproject.handlers.process_data

# Check architecture health
pyscope nav health

# Export to Mermaid for docs
pyscope query export --format mermaid
```

After indexing, the graph is stored at `<project>/.pyscope/pyscope.json.gz`. All subsequent commands auto-discover and load it. Use `--graph <FILE>` to specify explicitly.

---

## Commands

### `pyscope index` — Build the Graph

```bash
pyscope index .                     # index current directory
pyscope index --no-tests .          # skip test files
pyscope index --output custom.json.gz  # custom output path
pyscope index --no-semantic .       # skip pyright LSP enrichment
```

### `pyscope query` — Explore the Graph

| Command | Purpose |
|---------|---------|
| `inspect <NAME>` | Symbol detail + source code (use `--no-source` to skip code) |
| `trace <NAME>` | Call chain: `--direction up/down/both --depth N` |
| `find <PATTERN>` | Text search (`--mode text`) or structural similarity (`--mode similar`) |
| `scope <TARGET>` | File scope (`--kind file`) or module scope (`--kind module`) |
| `impact <NAME>` | Full dependency impact: files affected, call sites, change hints |
| `symbols` | List symbols with 8 filter flags (`--dead`, `--no-docs`, `--min-callers`, ...) |
| `search <QUERY>` | Text search across names, signatures, docs |
| `source <NAME>` | Raw source code for a symbol |
| `similar <NAME>` | Find structurally similar symbols |
| `path <FROM> <TO>` | Shortest call path between two symbols |
| `export` | DOT / Mermaid / JSON export (`--format`) |

### `pyscope nav` — AI Navigation

| Command | Purpose |
|---------|---------|
| `map` | Token-budgeted project overview (`--full` for entries + clusters) |
| `health` | Architecture health: cycles, god modules, dead code |
| `entries` | Detected entry points |
| `clusters` | Feature clusters by file |
| `quality` | Graph quality score |
| `report` | Markdown report generation |
| `ask <QUESTION>` | LLM Q&A (requires API key) |
| `retrieve <QUERY>` | RAG retrieval: lexical → embedding → rerank |

### `pyscope analyze` — Static Analysis

| Command | Purpose |
|---------|---------|
| `deps` | Module dependency matrix + recompile impact |
| `fanout` | File-level fan-in / fan-out |
| `tests` | Call-graph-based test impact analysis |

### `pyscope config` — Settings

```bash
pyscope config show
pyscope config set-api-key "sk-..."
pyscope config set-model "gpt-4o"
pyscope config set-embedding-key "..."
```

Configuration stored at `~/.config/pyscope/config.json`.

---

## Architecture

```
pyscope/
├── src/
│   ├── model.rs          # CodeGraph, Node, Edge, NodeKind, EdgeKind
│   ├── cli.rs            # clap definitions: 6 top-level commands
│   ├── main.rs           # CLI entry, command dispatch
│   ├── store.rs          # Gzip JSON load/save
│   ├── config.rs         # LLM/RAG config (~/.config/pyscope/config.json)
│   ├── llm.rs            # OpenAI-compatible LLM client
│   ├── report.rs         # Markdown report generation
│   ├── analyzer/         # rustpython-parser AST indexer
│   │   ├── index.rs      # Main indexing pipeline (walkdir + parse + visit)
│   │   ├── visitors.rs   # Python AST walker: FunctionDef, ClassDef, Import, Call
│   │   ├── builder.rs    # Graph builder with fuzzy name resolution
│   │   └── helpers.rs    # Docstring extraction, visibility, line mapping
│   ├── query/            # Adjacency index + traversal + search
│   │   ├── index.rs      # QueryIndex: in-memory adjacency index
│   │   ├── find.rs       # Node resolution with Levenshtein suggestions
│   │   ├── traversal.rs  # BFS walk, path finding, node_value serialization
│   │   ├── commands.rs   # inspect, trace, find, scope, impact, search
│   │   ├── filter.rs     # SymbolFilter: 8 filter flags for symbols()
│   │   ├── source.rs     # Source code extraction by line range
│   │   ├── similar.rs    # Structural similarity: callee overlap + signature
│   │   └── export.rs     # DOT / Mermaid / JSON export
│   ├── nav/              # AI-oriented navigation
│   │   ├── map.rs        # Token-budgeted repository overview
│   │   └── health.rs     # Architecture health scoring
│   ├── analyze/          # Static analysis
│   │   ├── deps.rs       # Module dependency matrix
│   │   ├── fanout.rs     # File fan-in / fan-out
│   │   └── test_impact.rs # Call-graph test impact analysis
│   ├── semantic/         # pyright LSP enrichment
│   │   ├── mod.rs        # LSP subprocess + JSON-RPC
│   │   └── helpers.rs    # hover, references, signature formatting
│   └── rag/              # Retrieval pipeline
│       ├── lexical.rs    # Case-insensitive name/sig/docs search
│       ├── embedding.rs  # API embedding + cosine similarity
│       └── rerank.rs     # Hybrid reranking
└── tests/                # (to be added)
```

**Data flow:**
1. `walker` → `.py` files
2. `rustpython-parser` → Python AST
3. `visitors.rs` → `Builder` → `CodeGraph` (nodes + edges)
4. `store.rs` → `.pyscope/pyscope.json.gz`
5. `QueryIndex` → `walk` (BFS) → JSON output

**Module dependencies:** `model.rs` is the hub — all modules depend on it. `analyzer` builds the graph, `query` reads it, `nav` summarizes it, `analyze` audits it.

---

## Concepts

### Nodes

Each Python construct becomes a node: `function`, `method`, `class`, `module`, `file`, `variable`, `import`, `field`, `decorator`, `property`.

Each node carries: qualified name, file path, line range (start → end), full signature, docstring, visibility.

### Edges

Edges link nodes by relationship:
- `declares` — module → its symbols
- `calls` — function/method calls (with `call_style`: `direct` or `method`)
- `imports` — import statement → imported symbol
- `inherits_from` — class → base class
- `has_method` / `has_field` — class → its members
- `uses_type` — symbol → type it references
- `contains` — file → module

### Call Graph

`pyscope query trace` traverses the `calls` edges. A method call like `obj.process()` is detected as `call_style: "method"` — the parser sees `Expr::Call` where `func` is `Expr::Attribute`.

---

## Building

- **Rust** ≥ 1.85 (edition 2024)
- **No C library required** — pyscope is pure Rust

```bash
cargo build --release
```

---

⭐ Found this useful? Give it a star on GitHub.
