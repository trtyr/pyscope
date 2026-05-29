# Risk Hotspots

## Known Unknowns

1. **Dynamic typing** — Python has no type annotations required. Call resolution
   is best-effort: same-module first, then imported modules. Cannot resolve
   method calls on dynamically-typed variables.

2. **Nested functions** — Python supports deeply nested function definitions.
   Initial implementation may only index top-level and class-level functions.

3. **Comprehensions** — list/dict/set/generator comprehensions create implicit
   functions. Skip for MVP.

4. **Decorators** — `@decorator` wraps the decorated function. Capture as
   `Decorator` edge; don't try to resolve the transformed call chain.

5. **Import resolution** — `from foo.bar import baz` requires finding `foo/bar.py`
   or `foo/bar/__init__.py`. File-system-based resolution only; no
   sys.path or virtualenv logic.

6. **Type stubs** — `.pyi` files skipped for MVP.

7. **Large projects** — 1000+ file projects untested for indexing performance.

## Architecture Risks

1. **Call resolution accuracy** — Python's dynamic nature means the call graph
   will always be incomplete. Frame the output as "best-effort static analysis"
   rather than "complete call graph."

2. **rustpython-parser maturity** — Check for edge cases: f-strings, match
   statements (3.10+), type parameter syntax (3.12+).
