# Test and Release Gates

## Test Strategy

- **Integration tests**: via `std::process::Command` invoking the built binary
- **Test fixture**: `tests/fixtures/sample/` — minimal Python project with:
  - Top-level functions
  - Classes with methods
  - Decorators
  - Imports (local and stdlib)
  - Nested functions
- **Test assertions**: verify JSON output structure, node counts, edge types

## Verification Gates

```
cargo test            # all tests pass
cargo build --release # clean compile
cargo run -- index tests/fixtures/sample/  # indexes without errors
cargo run -- query inspect sample_function  # returns valid JSON
```

## Target Test Project

[ppt-master](https://github.com/user/ppt-master) — 100+ Python files, real-world complexity. Used for integration testing and performance validation.
