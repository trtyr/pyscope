# Runtime Flows

## Index Pipeline

```
cargo run -- index /path/to/python/project
  ↓
main.rs: index_project(path)
  ↓
analyzer/index.rs:
  1. Discover .py files (walk directory, respect __pycache__ skip)
  2. For each .py file:
     a. Read source
     b. Parse with rustpython-parser → ast::Suite
     c. Walk AST via visitors.rs → extract symbols, calls, imports
     d. Feed into builder.rs → nodes + edges + pending resolution
  3. Resolve pending edges (cross-file imports, call targets)
  4. Serialize to .pyscope/pyscope.json.gz via store.rs
```

## Query Pipeline

```
cargo run -- query inspect some_function
  ↓
main.rs: query dispatch
  ↓
store.rs: load .pyscope/pyscope.json.gz → CodeGraph
  ↓
query/commands.rs: inspect(graph, name)
  ↓
query/find.rs: find_nodes(graph, name) → resolved node
  ↓
query/index.rs: QueryIndex for adjacency lookups
  ↓
Return JSON: node info + incoming/outgoing edges + source code
```

## AST Parsing (rustpython-parser)

```rust
use rustpython_parser::{parse, Mode};

let source = std::fs::read_to_string("file.py")?;
let ast = parse(source, Mode::Module, "<filename>")?;
// ast is a Suite (Vec<Stmt>)
// Walk statements: Stmt::FunctionDef, Stmt::ClassDef, Stmt::Import, etc.
```
