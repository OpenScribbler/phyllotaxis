# CLAUDE.md

Progressive disclosure CLI for OpenAPI specs. Primary audience is LLMs — output formats and information density should optimize for feeding into prompts, not human scanning.

## Build & CI

All four CI gates must pass locally before pushing:

```bash
cargo test --locked              # 232+ integration & unit tests
cargo clippy --locked -- -D warnings  # zero warnings policy
cargo fmt --check                # formatting must match exactly
cargo deny check                 # license & advisory audit
```

Always use `--locked` with cargo build/test/clippy to match CI behavior.

## Testing

Tests live in `tests/integration_tests.rs` (integration) and `tests/lib_tests.rs` (unit). Integration tests run the compiled binary and assert on stdout/stderr/exit code.

Two fixture helpers exist — use the right one:
- `run_with_petstore(&[...])` — standard API, use for most command tests
- `run_with_kitchen_sink(&[...])` — edge cases (callbacks, complex schemas, composition patterns)

Pattern for new tests:
```rust
#[test]
fn test_commandname_scenario() {
    let (stdout, _stderr, code) = run_with_petstore(&["command", "arg"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("expected output"), "descriptive failure message");
}
```

When adding a new command or fixing a bug, add integration tests that cover both text and `--json` output modes.

## Error Handling

All errors must flow through `anyhow::bail!()` and propagate to the single handler in `main()`. Never use `eprintln!()` + `process::exit(1)` — this was a bug that caused duplicate error messages.

Never use `.expect()` or `.unwrap()` on user-facing paths (file I/O, path conversion, env vars). Use `.with_context()` or `.ok_or_else(|| anyhow!(...))` instead. Panics are bugs, not error handling.

## Output Modes

Every command supports `--json` for machine consumption. When modifying command output:
- Update both `src/render/text.rs` and `src/render/json.rs`
- Text mode is human-readable; JSON mode is structured for LLM consumption
