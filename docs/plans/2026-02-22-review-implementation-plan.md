# Review Implementation Plan — phyllotaxis
**Date:** 2026-02-22
**Source:** Multi-agent review (security, UX, accessibility, Rust code quality)
**Reports:** `docs/reports/rust-{security,ux,accessibility,code}-review-report.md`

---

## Overview

~50 findings across four dimensions. Organized into four sequential phases:
- **Phase 1** — No-risk quick wins (Cargo.toml, config)
- **Phase 2** — Structural foundation (lib.rs, error handling)
- **Phase 3** — Behavior changes (visible to users)
- **Phase 4** — Polish and tooling

Execute phases in order. Phase 2 must complete before Phase 3 (error propagation refactor touches the same paths as behavior changes).

## TDD Approach

All work follows test-driven development, adapted to the task type:

- **Refactoring tasks (Phase 2, Phase 4):** Characterize → Refactor → Green
  1. Run existing tests to identify coverage gaps
  2. Write tests that characterize current behavior before changing anything (these pass initially)
  3. Make the structural change
  4. All tests still pass — regressions caught immediately

- **New behavior tasks (Phase 3):** Red → Green → Refactor
  1. Write a failing test for the desired behavior
  2. Implement the behavior
  3. Test passes

No task in Phase 2 or 3 is considered complete until its tests pass.

---

## Phase 1 — Quick Wins (No behavior change, no risk)

### 1.1 Replace `serde_yaml` with `serde_yaml_ng`

**Source:** Security Finding 3 (High), Code research
**Files:** `Cargo.toml`, `src/spec.rs`

`serde_yaml 0.9` was archived by its author in 2024. The fork `serde_yml` has a confirmed unsoundness advisory (RUSTSEC-2025-0068). `serde_yaml_ng` is the actively maintained drop-in replacement.

**Tasks:**
1. In `Cargo.toml`, replace `serde_yaml = "0.9"` with `serde_yaml_ng = "0.9"`
2. In `src/spec.rs`, replace all `use serde_yaml::` with `use serde_yaml_ng::`
3. Run `cargo build` and `cargo test` to verify no breakage

**Expected diff size:** ~5 lines

---

### 1.2 Add `[profile.release]` optimizations

**Source:** Security Finding 9, Code Finding (Low)
**Files:** `Cargo.toml`

No release profile exists. The distributed binary includes debug info and has integer overflow checks disabled.

**Tasks:**
1. Add the following section to `Cargo.toml`:
   ```toml
   [profile.release]
   lto = true
   codegen-units = 1
   strip = true
   overflow-checks = true
   panic = "abort"
   ```

**Notes:**
- `overflow-checks = true` catches integer wrapping bugs in release builds
- `strip = true` removes debug symbols for smaller binaries
- `panic = "abort"` is appropriate for a CLI (no unwinding needed); reduces binary size

---

### 1.3 Verify `Cargo.lock` is tracked in git

**Source:** Security Finding 12
**Files:** `.gitignore` (if present), git history

For binary crates (not libraries), `Cargo.lock` must be committed to ensure reproducible builds and auditable dependency trees.

**Tasks:**
1. Check `.gitignore` — ensure `Cargo.lock` is not listed
2. Confirm `Cargo.lock` is staged: `git add Cargo.lock`

---

## Phase 2 — Structural Foundation

These changes don't alter user-visible behavior but enable Phase 3 and improve testability and maintainability throughout.

### 2.1 Add `src/lib.rs` re-export hub

**Source:** Code Finding 1 (High)
**Files:** `src/lib.rs` (new), `src/main.rs`

Without `lib.rs`, the `tests/` directory cannot access internal functions without spawning subprocesses. The existing integration tests work around this by re-parsing fixtures from scratch or calling `std::process::Command`. Adding `lib.rs` enables direct unit testing of `spec::load_spec`, `commands::*`, and `render::*`.

**Tasks:**
1. Create `src/lib.rs` that re-exports the internal modules:
   ```rust
   pub mod spec;
   pub mod models;
   pub mod commands;
   pub mod render;
   ```
2. Remove duplicate `mod` declarations from `src/main.rs` (replace with `use phyllotaxis::*` or keep explicit)
3. Update `Cargo.toml` if needed (no changes required — Cargo auto-detects `lib.rs`)
4. Run `cargo test` to verify existing tests still pass

**TDD steps:**
1. Before creating `lib.rs`, write tests in `tests/` that directly call internal functions (e.g., `phyllotaxis::spec::load_spec(...)`, `phyllotaxis::commands::resources::extract_resource_groups(...)`). These will fail to compile — that's the red state.
2. Create `src/lib.rs` to make them compile and pass.
3. Run full test suite — all 86 existing tests must still pass.

---

### 2.2 Migrate `spec.rs` errors from `Result<_, String>` to `anyhow`

**Source:** Code Finding 2 (High), UX research
**Files:** `src/spec.rs`, `src/main.rs`

`Result<_, String>` discards error context, prevents callers from pattern-matching on error types, and makes test assertions fragile (`assert!(err.contains("Failed to parse"))`). `anyhow` replaces this with zero boilerplate while adding `.context()` for rich error chains.

**Tasks:**
1. Add `anyhow = "1"` to `[dependencies]` in `Cargo.toml`
2. In `src/spec.rs`:
   - Add `use anyhow::{Context, Result};`
   - Change `fn resolve_spec_path(...) -> Result<PathBuf, String>` to `-> anyhow::Result<PathBuf>`
   - Change `fn load_spec(...) -> Result<LoadedSpec, String>` to `-> anyhow::Result<LoadedSpec>`
   - Replace `Err(format!(...))` with `Err(anyhow::anyhow!(...))` or use `?` with `.context()`
   - Replace `.map_err(|e| format!(...))` with `.with_context(|| format!(...))`
3. In `src/main.rs`, update the `match spec::load_spec(...)` call — the type changes, but the `die(&e)` usage can be updated to `die(&format!("{e:#}"))` to preserve chain formatting
4. Update any tests that assert on error string contents

**TDD steps:**
1. Read existing error-path tests. Find assertions using `.contains("...")` substring matching on error strings — these are fragile and need strengthening.
2. Before touching `spec.rs`, update those tests to assert on the full expected error message (not substrings). Tests should still pass at this point.
3. Migrate to `anyhow`. Tests still pass — same messages, now via error chains.
4. Update any tests that legitimately need to change due to improved error context (e.g., `{:#}` formatting adds chain context).

---

### 2.3 Replace `die()` with proper error propagation

**Source:** Code Finding 3 (High)
**Files:** `src/main.rs`

`die()` terminates the process from inside match arms, uses exit code 1 uniformly for all errors (correct exit codes differ: parse failure vs not-found vs bad args), and is applied inconsistently (some not-found paths inline `process::exit(1)` directly).

**Tasks:**
1. Make `main()` return `anyhow::Result<()>` or a custom `ExitCode`
2. Thread errors back to `main()` via `?` instead of calling `die()` or `process::exit()`
3. In `main()`, match on error type to produce correct exit codes:
   - Spec not found / parse failure: exit 1
   - Resource/schema not found: exit 1 with "Did you mean" to stderr
   - Bad args (handled by clap): already exit 2
4. Delete the `die()` function
5. Add `human-panic` (optional, see Phase 4) to replace raw panic output

**Notes:** This task can be scoped incrementally — convert one command at a time, replace `die()` last when all callers are updated.

**TDD steps:**
1. Write integration tests (using `assert_cmd`) that assert specific exit codes for each error scenario:
   - Spec file not found → exit code 1
   - Spec file found but invalid YAML → exit code 1
   - Resource name not found → exit code 1 (with suggestion on stderr)
   - Schema name not found → exit code 1 (with suggestion on stderr)
2. Run them against current code — verify they pass (or note which don't, and fix the exit code behavior as part of this task).
3. Remove `die()` and thread errors to `main()`.
4. All exit code tests still pass.

---

## Phase 3 — User-Visible Behavior Changes

### 3.1 ANSI escape injection sanitization

**Source:** Security Finding 1 (High), Accessibility Finding (High)
**Files:** `src/render/text.rs`, possibly `src/render/json.rs`

Every text-mode render function interpolates spec-sourced strings (descriptions, paths, enum values, auth scheme names) directly into `format!()` calls. A crafted OpenAPI spec can inject ANSI escape sequences that manipulate the developer's terminal (cursor repositioning, color bleeding, title-bar injection).

**Tasks:**
1. Create a sanitization function:
   ```rust
   fn sanitize_for_terminal(s: &str) -> String {
       s.chars()
        .filter(|c| !matches!(*c as u32, 0x1B | 0x07 | 0x08 | 0x0C))
        .collect()
   }
   ```
   Or prefer a crate: `strip-ansi-escapes` for stripping codes already present in the input.
2. Apply `sanitize_for_terminal()` to all spec-sourced strings in `render/text.rs` before interpolation:
   - Descriptions (resource, endpoint, schema, parameter)
   - Enum values
   - Auth scheme names, descriptions
   - Discriminator mapping values
   - Any `spec_title`, `spec_version` fields
3. The JSON renderer does not need this (JSON encoding handles escaping naturally)

**TDD steps:**
1. Create a test fixture OpenAPI spec YAML file containing ANSI escape sequences embedded in field descriptions (e.g., `description: "Normal text \x1b[31mRED\x1b[0m injected"`).
2. Write a test that renders it in text mode and asserts the output does NOT contain the ESC character (`\x1b`, `0x1B`).
3. Test fails — no sanitization exists yet.
4. Implement `sanitize_for_terminal()` and apply it.
5. Test passes.

**Options:**
- **Option A:** Write a minimal sanitizer inline (strips ESC and control chars) — no new dependency
- **Option B:** Add `strip-ansi-escapes = "0.2"` crate — more comprehensive, handles partial sequences
- **Option C:** Add `anstream` — handles sanitization + TTY detection together (see Finding 3.2)

---

### 3.2 TTY detection, NO_COLOR, and CLICOLOR compliance

**Source:** UX Finding 6 (Medium), Accessibility Findings 1 & 2 (High)
**Files:** `src/main.rs`, `src/render/text.rs`

No TTY detection exists. Formatting (headers, "Drill deeper" footers, separators, Unicode arrows) is emitted unconditionally whether stdout is a terminal or a pipe. The NO_COLOR environment variable is not checked.

**Note:** The current codebase produces no ANSI color codes today, so there is no active failure. But this is a safety net for future color additions and affects the piped output use case.

**Tasks:**
1. Add `is-terminal` to dependencies (or use `std::io::IsTerminal` from stdlib, stable since Rust 1.70 — no crate needed)
2. Pass a `is_tty: bool` flag through the render layer, or use a global (thread-local) flag set in `main()`
3. In `main()`, detect TTY and check env vars:
   ```rust
   use std::io::IsTerminal;
   let is_tty = std::io::stdout().is_terminal()
       && std::env::var("NO_COLOR").is_err()
       && std::env::var("TERM").as_deref() != Ok("dumb");
   ```
4. In `render/text.rs`, gate decorative elements (headers, separators, "Drill deeper" sections) on `is_tty`
5. When `!is_tty`, emit plain `Key: value` output instead of columnar/padded layout

**TDD steps:**
1. Write tests using `assert_cmd` that set `NO_COLOR=1` in the environment and assert that decorative headers/footers ("Drill deeper:", separator lines) are absent from output.
2. Write a test that pipes output (stdout is not a TTY in test context) and asserts plain `Key: value` format instead of padded columnar layout.
3. Tests fail — no TTY detection exists.
4. Implement detection and gating.
5. Tests pass.

**Options:**
- **Option A:** Pass `is_tty: bool` to each render function — simple, explicit, testable
- **Option B:** Add `colorchoice-clap` crate — handles NO_COLOR + CLICOLOR_FORCE + TTY in one `#[arg]` on the CLI struct, recommended if color support will be added later
- **Option C:** Use `anstream` — wraps stdout/stderr with automatic stripping; pairs well with Option C from 3.1

---

### 3.3 JSON mode: structured errors and compact output

**Source:** UX Finding 3 (High), UX Finding 2 (High)
**Files:** `src/main.rs`, `src/render/json.rs`

Two issues:
1. Errors in `--json` mode emit plain `eprintln!` text, not JSON — breaks scriptability
2. All JSON output is unconditionally pretty-printed — incorrect when piped

**Tasks:**

**Subtask A — JSON errors:**
1. Create a helper:
   ```rust
   fn json_error(msg: &str) -> String {
       serde_json::json!({"error": msg}).to_string()
   }
   ```
2. In `--json` mode, replace all `eprintln!("Error: ...")` + `process::exit(1)` with `eprintln!("{}", json_error(&msg))` + `process::exit(1)`
3. Not-found paths in `main.rs:128–135` and `main.rs:171–179` need this treatment
4. The `die()` function (if not yet replaced by Phase 2.3) needs a JSON variant

**Subtask B — Compact JSON when piped:**
1. Use TTY detection from 3.2 to select pretty vs compact:
   ```rust
   if is_tty {
       serde_json::to_string_pretty(&data)
   } else {
       serde_json::to_string(&data)
   }
   ```
2. Apply in all `render/json.rs` render functions

**TDD steps (Subtask A — JSON errors):**
1. Write an integration test that runs `phyllotaxis --json resources nonexistent` and asserts:
   - Exit code is 1
   - stderr is valid JSON containing an `"error"` key
   - stderr does NOT contain plain `"Error: ..."` text
2. Test fails.
3. Implement JSON error output.
4. Test passes.

**TDD steps (Subtask B — compact JSON):**
1. Write a test that captures stdout of `phyllotaxis --json resources` in a non-TTY context and asserts the output is compact JSON (no leading spaces, no newlines within objects).
2. Test fails.
3. Implement compact-when-piped logic.
4. Test passes.

---

### 3.4 YAML injection in `init` command

**Source:** Security Finding 2 (High)
**Files:** `src/commands/init.rs:168, 231`

User-provided file paths and spec names are interpolated raw into YAML config content via `format!()`. A path or name containing a newline followed by YAML structure can inject keys into the written config.

**Tasks:**
1. Use `serde_yaml_ng` (already added in Phase 1.1) to serialize the config struct rather than building YAML by hand with `format!()`
2. Define a `Config` struct:
   ```rust
   #[derive(Serialize)]
   struct Config {
       spec: String,
       // other fields
   }
   ```
3. Serialize with `serde_yaml_ng::to_string(&config)?` instead of string interpolation
4. This also fixes the `run_add_spec` line-splitting heuristic (UX Finding 8)

**TDD steps:**
1. Write a test that calls the init write logic with a spec path containing a newline followed by YAML structure (e.g., `"legit/path.yaml\ninjected_key: injected_value"`).
2. Parse the resulting `.phyllotaxis.yaml` and assert it does NOT contain `injected_key`.
3. Test fails against the current string-interpolation approach.
4. Implement struct-based serialization.
5. Test passes.

---

### 3.5 Atomic writes in `init` command

**Source:** Security Finding 4 (Medium)
**Files:** `src/commands/init.rs`

Config writes open and truncate the file, then write content. A crash mid-write produces a corrupt config. Atomic write pattern: write to a temp file in the same directory, then `fs::rename()`.

**Tasks:**
1. Use `tempfile` (already a dev-dependency — promote to regular dependency or use stdlib):
   ```rust
   let tmp_path = config_path.with_extension("tmp");
   fs::write(&tmp_path, content)?;
   fs::rename(&tmp_path, &config_path)?;
   ```
2. Alternatively, use `tempfile::NamedTempFile::persist()` for the same guarantee
3. Ensure temp file is in the same directory as the target (required for rename to be atomic on Linux)

**TDD steps:**
1. Write a test that performs an init write and asserts no `.phyllotaxis.tmp` file remains afterward (verifying cleanup on success).
2. The test may pass trivially with the current approach — the point is to ensure the atomic pattern is verified and a regression would be caught if the rename step were accidentally removed.
3. Implement atomic write pattern.
4. Test passes.

---

### 3.6 `init` non-interactive mode

**Source:** UX Finding 7 (Medium)
**Files:** `src/commands/init.rs`, `src/main.rs`

`init` unconditionally reads from stdin. It hangs silently in CI environments. No `--spec` flag equivalent exists for scripted use.

**Tasks:**
1. Add a `--spec-path` flag to the `Init` subcommand:
   ```rust
   Init {
       /// Spec file path (skips interactive prompt)
       #[arg(long)]
       spec_path: Option<PathBuf>,
   }
   ```
2. In `run_init()`, if `spec_path` is `Some`, skip the interactive prompt and proceed directly
3. Document the non-interactive pattern in `--help`

**TDD steps:**
1. Write an integration test that runs `phyllotaxis init --spec-path path/to/spec.yaml` with stdin closed (non-interactive). Assert it completes without blocking and writes a valid `.phyllotaxis.yaml`.
2. Test fails — `--spec-path` flag does not exist.
3. Add the flag and implement the non-interactive path.
4. Test passes.

---

### 3.7 Unicode arrow semantic replacement

**Source:** Accessibility Finding 4 (Medium)
**Files:** `src/render/text.rs:148, 325–328`

`→` (U+2192) conveys the meaning "maps to" for response schemas and discriminator mappings. Screen readers read it as "right arrow," breaking sentence comprehension. Fails silently to `?` in non-UTF-8 terminals.

**Tasks:**
1. Replace `→` with text labels:
   - Response schema reference: `(schema: PetResponse)` instead of `→ PetResponse`
   - Discriminator mappings: `maps to: PetCat` instead of `→ PetCat`
2. Or gate on TTY: use `→` in TTY mode, `->` or text label in non-TTY mode (aligns with 3.2)

**TDD steps:**
1. Write a test using a spec fixture that has response schema references.
2. In non-TTY mode, assert the rendered output does NOT contain the `→` character (U+2192).
3. Assert the output DOES contain a text label conveying the same relationship (e.g., `schema:` or `->`).
4. Test fails.
5. Implement the replacement.
6. Test passes.

---

## Phase 4 — Polish and Tooling

### 4.1 `strsim` for typo suggestions

**Source:** UX Finding 5 (Medium)
**Files:** `src/commands/resources.rs:543`, `src/commands/schemas.rs:55`

Current suggestion matching uses substring containment — it misses common typos (transpositions, deletions). `strsim` provides Jaro-Winkler distance, which is what `clap` itself uses.

**Tasks:**
1. Add `strsim = "0.11"` to dependencies
2. Replace the `contains()` check in `suggest_similar` and `suggest_similar_schemas` with Jaro-Winkler distance filtering:
   ```rust
   use strsim::jaro_winkler;
   names.iter()
       .filter(|n| jaro_winkler(query, n) > 0.8)
       .cloned()
       .collect()
   ```

**TDD steps:**
1. Write tests with "near miss" inputs that substring matching misses: e.g., `"petss"` → should suggest `"pets"`, `"usrs"` → should suggest `"users"`.
2. Assert current behavior does NOT suggest correctly (documenting the gap).
3. Implement `strsim` distance filtering.
4. Assert tests now pass with correct suggestions.

---

### 4.2 Shell completions via `clap_complete`

**Source:** UX research (Low)
**Files:** `src/main.rs` or a dedicated `build.rs`

Shell completions are a high-value, low-effort addition for developer tooling.

**Tasks:**
1. Add `clap_complete = "4"` to dependencies
2. Add a hidden `completions` subcommand or generate completions at build time via `build.rs`
3. Support at minimum: `bash`, `zsh`, `fish`

---

### 4.3 Add CI/CD pipeline with `cargo audit`

**Source:** Security Finding 10 (Medium)
**Files:** `.github/workflows/` (new)

No CI exists. No automated audit runs. New vulnerabilities in dependencies will not be detected.

**Tasks:**
1. Create `.github/workflows/ci.yml`:
   - `cargo build`
   - `cargo test`
   - `cargo clippy -- -D warnings`
   - `cargo audit` (via `rustsec/audit-check` action)
2. Schedule weekly advisory checks separately from PR checks

---

### 4.4 `human-panic` for clean panic output

**Source:** UX research (Low)
**Files:** `src/main.rs`

Raw Rust panics expose backtraces and internal module paths to end users. `human-panic` replaces this with a friendly message and a temp file containing the panic details.

**Tasks:**
1. Add `human-panic = "2"` to dependencies
2. Add to top of `main()`:
   ```rust
   human_panic::setup_panic!();
   ```

---

### 4.5 `push_str(&format!(...))` → `write!()`

**Source:** Code Finding (Medium)
**Files:** `src/render/text.rs`

51 instances of `push_str(&format!(...))` allocate a temporary `String` on each call. `write!(out, ...)` on a `String` avoids the intermediate allocation and is the idiomatic form.

**Tasks:**
1. Add `use std::fmt::Write;` to `render/text.rs`
2. Replace `out.push_str(&format!("...", args))` with `writeln!(out, "...", args).unwrap()` or `let _ = write!(out, ...)` throughout
3. This is a mechanical transformation — run with Clippy's `useless_format` lint to find candidates

**TDD note:** This is a pure mechanical refactor — behavior does not change. No new tests needed. The existing render output tests serve as the regression suite. Run full test suite before and after to confirm no output changes.

---

### 4.6 Decompose large functions

**Source:** Code Finding (Medium)
**Files:** `src/main.rs`, `src/commands/resources.rs`, `src/render/json.rs`

- `main()` — 143 lines; extract command dispatch into per-command handler functions
- `get_endpoint_detail` — 212 lines with 5 distinct responsibilities
- `render_schema_detail` in `json.rs` — 5 struct definitions + nested helper inside function body

**Tasks:**
1. In `main.rs`, extract each `Some(Commands::X)` arm into `handle_x(cli, loaded)` functions
2. In `resources.rs`, split `get_endpoint_detail` into: resolve endpoint, extract parameters, extract request body, extract responses, build model
3. In `json.rs`, promote the 5 inner structs to module-level, extract `convert_fields` as a module-level function

**TDD note:** Function decomposition is a refactor — external behavior is unchanged. Existing integration tests serve as the regression suite. Run full test suite before and after to confirm no regressions.

---

## Execution Order Summary

```
Phase 1 (now):     1.1 → 1.2 → 1.3
Phase 2 (next):    2.1 → 2.2 → 2.3   (in order — each unblocks the next)
Phase 3 (then):    3.1, 3.4, 3.5 first (security)
                   3.2, 3.3, 3.6, 3.7 after (behavior)
Phase 4 (polish):  any order, as bandwidth allows
```

## Files Most Affected

| File | Phases |
|------|--------|
| `Cargo.toml` | 1.1, 1.2, 2.2, 4.1–4.4 |
| `src/main.rs` | 2.3, 3.2, 3.3, 4.4, 4.6 |
| `src/spec.rs` | 1.1, 2.2 |
| `src/commands/init.rs` | 3.4, 3.5, 3.6 |
| `src/render/text.rs` | 3.1, 3.2, 3.7, 4.5 |
| `src/render/json.rs` | 3.3, 4.6 |
| `src/commands/resources.rs` | 4.1, 4.6 |
| `src/lib.rs` (new) | 2.1 |
