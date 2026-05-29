# Open Questions

## Architecture

1. **Reuse crabmap model.rs directly?** Copy-paste-adapt vs. extract shared crate. Copy-paste-adapt is simpler for MVP; both projects are small enough that duplication isn't a maintenance burden yet.

2. **NodeKind definitions** — include Python-specific variants (AsyncFunction, Generator, Lambda) or keep NodeKind generic with metadata fields? → Use Python-specific variants to make the graph self-describing.

3. **rustpython-parser v0.4.0** — verify that it supports Python 3.12 syntax (type params, match statements) before committing.

## Test Project

4. **ppt-master location** — where is this project cloned? Need absolute path for integration testing.
