# Module Map

Architecture: adapt crabmap's model/query/store, write Python-specific analyzer.

## Module Layout

```
pyscope/
├── src/
│   ├── main.rs          # CLI entry, command dispatch
│   ├── cli.rs           # clap definitions
│   ├── model.rs         # CodeGraph, Node, Edge, NodeKind, EdgeKind
│   ├── analyzer/        # Python-specific AST indexer
│   │   ├── mod.rs
│   │   ├── index.rs     # Main indexing: walk .py files, parse, build graph
│   │   ├── builder.rs   # Graph builder: nodes + edges + pending resolution
│   │   ├── helpers.rs   # Doc extraction, name resolution, metrics
│   │   └── visitors.rs  # AST visitor: extract symbols, calls, imports
│   ├── query/           # Graph query infrastructure
│   │   ├── mod.rs
│   │   ├── index.rs     # QueryIndex: adjacency index for fast lookups
│   │   ├── find.rs      # Node resolution: find by name, fuzzy matching
│   │   ├── traversal.rs # Graph traversal: walk, path, neighbors
│   │   └── commands.rs  # High-level query commands
│   └── store.rs         # Gzip JSON load/save
└── tests/
    ├── integration.rs
    └── fixtures/
        └── sample/      # Minimal Python project for test indexing
```

## Key Differences from crabmap

| Concept | crabmap (Rust) | pyscope (Python) |
|---------|---------------|------------------|
| Primary type | struct | class |
| Sum types | enum | (no native equivalent) |
| Interfaces | trait | ABC / Protocol / duck typing |
| Functions | fn (top-level) | def (top-level or nested) |
| Methods | impl block | def inside class |
| Imports | use | import / from ... import |
| Visibility | pub, pub(crate) | _ prefix convention |
| Decorators | proc macros | @decorator |
| Async | async fn | async def |
| Package | Cargo.toml | __init__.py |

## NodeKind (Python-specific)

Adapted from crabmap's NodeKind — Python doesn't have struct/enum/trait/impl:

```
Project, File, Module, Package,
Function, Method, AsyncFunction, AsyncMethod,
Class, ClassMethod, StaticMethod,
Variable, Field, Property, Decorator,
Import, Unknown
```

## EdgeKind

```
Contains, Declares, Imports, Calls, UsesType,
HasMethod, InheritsFrom
```

## Dependency Direction

```
model.rs ← (all modules)
analyzer/ → model.rs only
query/ → model.rs only
store.rs → model.rs only
main.rs → (all modules via dispatch)
cli.rs — dependency-free
```
