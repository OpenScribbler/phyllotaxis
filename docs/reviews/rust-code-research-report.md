# Rust Code Quality & Idiomatic Patterns: Research Report

**Project:** phyllotaxis — a Rust CLI for progressive disclosure of OpenAPI specs
**Date:** 2026-02-21
**Scope:** Code quality, idiomatic patterns, and best practices for CLI applications in Rust

---

## Table of Contents

1. [Error Handling](#1-error-handling)
2. [Idiomatic Patterns: Option, Result, Iterators](#2-idiomatic-patterns-option-result-iterators)
3. [Unwrap and Panic — When Is It Acceptable?](#3-unwrap-and-panic--when-is-it-acceptable)
4. [Module Organization](#4-module-organization)
5. [Clippy and Lints](#5-clippy-and-lints)
6. [Testing](#6-testing)
7. [Performance and Allocation](#7-performance-and-allocation)
8. [Type Design](#8-type-design)
9. [Lifetime and Ownership Patterns](#9-lifetime-and-ownership-patterns)
10. [Documentation](#10-documentation)
11. [Edition 2021 Features](#11-edition-2021-features)
12. [Cargo Best Practices](#12-cargo-best-practices)
13. [CLI-Specific Patterns: Separating IO from Logic](#13-cli-specific-patterns-separating-io-from-logic)

---

## 1. Error Handling

### The Two Libraries: `thiserror` vs `anyhow`

The Rust community has largely converged on two complementary error-handling crates: `thiserror` for defining structured error types and `anyhow` for ergonomic error propagation in applications. The choice between them is not library vs. application — it is really about whether you need to _handle_ errors programmatically or merely _report_ them to the user.

**`anyhow`** provides a single opaque `anyhow::Error` type that can wrap any `std::error::Error`. Its `context()` method adds human-readable context to errors without requiring custom enum variants. For CLI tools that primarily display errors to users and exit, this is the right default:

```rust
use anyhow::{Context, Result};

fn load_spec(path: &Path) -> Result<OpenApi> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read spec file: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse spec file: {}", path.display()))
}
```

**`thiserror`** is a derive macro that reduces boilerplate when defining typed error enums. Use it when callers need to match on specific error variants:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpecError {
    #[error("file not found: {path}")]
    NotFound { path: PathBuf },
    #[error("parse error: {0}")]
    Parse(#[from] serde_yaml::Error),
}
```

**Recommendation for phyllotaxis:** Add `anyhow` as a dependency. Replace the current `Result<_, String>` pattern in `spec.rs` with `anyhow::Result`. The `String` error type discards structured error information and provides no error chain. `anyhow` gives you `context()` chaining, better debug output, and a clean `main()`:

```rust
fn main() -> anyhow::Result<()> {
    // errors automatically display with full chain on exit
}
```

A common hybrid pattern for larger CLIs: use `thiserror` to define typed internal errors for logic that might need matching (e.g., "spec not found" vs. "spec parse failure"), then let `anyhow` convert them at the boundary for user-facing reporting.

**Sources:**
- [Rust Error Handling: thiserror, anyhow, and When to Use Each](https://momori.dev/posts/rust-error-handling-thiserror-anyhow/)
- [Rust Error Handling Compared: anyhow vs thiserror vs snafu](https://leapcell.medium.com/rust-error-handling-compared-anyhow-vs-thiserror-vs-snafu-597383d81c25)
- [Error Handling for Large Rust Projects — GreptimeDB](https://greptime.com/blogs/2024-05-07-error-rust)

---

## 2. Idiomatic Patterns: Option, Result, Iterators

### Prefer `?` Over Manual Matching

The `?` operator is the idiomatic way to propagate errors. It eliminates noise and keeps the happy path readable. Avoid writing `match result { Ok(v) => v, Err(e) => return Err(e) }` when `result?` achieves the same thing.

### Use Iterator Combinators

Rust iterators are lazy and zero-cost. Chains of `.filter()`, `.map()`, `.flat_map()` compile to the same code as hand-written loops — the compiler performs automatic loop fusion. Prefer iterator chains over explicit `for` loops when the transformation is expressible as a pipeline:

```rust
// Idiomatic
let names: Vec<&str> = endpoints
    .iter()
    .filter(|ep| ep.method == "GET")
    .map(|ep| ep.name.as_str())
    .collect();

// Prefer filter_map over filter + map when both are needed:
let valid_ids: Vec<u32> = raw_ids
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect();
```

**Key iterator advice:**
- Use `filter_map` instead of `filter().map()` when the filter condition is the transformation result
- Avoid calling `collect()` if you only need to iterate once more — pass the iterator directly
- Use `extend()` to populate an existing collection rather than `collect()` + `append()`
- Prefer `&str` over `&String` in function parameters (borrow the borrowed type, not the owned type)

### `Option` Combinators

Avoid `match` on `Option` when a combinator is clearer:

```rust
// Instead of:
match name {
    Some(n) => Some(n.to_uppercase()),
    None => None,
}
// Write:
name.map(|n| n.to_uppercase())

// Instead of:
if let Some(v) = foo() { v } else { default }
// Write:
foo().unwrap_or(default)
foo().unwrap_or_else(|| expensive_default())
```

**Sources:**
- [Processing a Series of Items with Iterators — The Rust Programming Language](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- [Iterators — The Rust Performance Book](https://nnethercote.github.io/perf-book/iterators.html)
- [Idioms — Rust Design Patterns](https://rust-unofficial.github.io/patterns/idioms/)

---

## 3. Unwrap and Panic — When Is It Acceptable?

The community position, well-articulated by BurntSushi in ["Using unwrap() in Rust is Okay"](https://burntsushi.net/unwrap/), is that `unwrap` is fine when panicking indicates a bug, not an expected failure condition.

**Acceptable uses of `unwrap`/`expect`:**
- **In tests** — panics are the expected failure mechanism; test runners catch them
- **Logically-guaranteed values** — when you can prove at the call site that the value is valid (e.g., a hardcoded valid IP address parsed via `"127.0.0.1".parse().unwrap()`)
- **Prototype/example code** — before error handling is wired up
- **Contract violations** — when the caller has violated an invariant that represents a bug, not a runtime condition

**Always avoid `unwrap`/`expect` for:**
- User input that might be invalid
- I/O operations (file reads, network, env vars)
- Any situation where the caller could reasonably encounter the failure

**Prefer `expect` over `unwrap`** when you must use either. The string message documents intent and produces a more useful panic message. Even better: give the message enough context that a developer can find the call site:

```rust
// Bad
env::var("HOME").unwrap()

// Better
env::var("HOME").expect("HOME env var not set")

// Best: propagate with ?
env::var("HOME").context("HOME env var not set")?
```

**For phyllotaxis specifically:** The `main.rs` uses `.expect("cannot determine current directory")` for `current_dir()` — this is appropriate since a missing working directory is a system-level bug. The `.expect("spec path not valid UTF-8")` on path conversion is a reasonable contract assertion. However, any `unwrap()`/`expect()` on user-supplied data should be replaced with `?` and proper error propagation.

**Sources:**
- [To panic! or Not to panic! — The Rust Programming Language](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html)
- [Using unwrap() in Rust is Okay — BurntSushi](https://burntsushi.net/unwrap/)
- [Replacing unwrap() and avoiding panics in Rust](https://klau.si/blog/replacing-unwrap-and-avoiding-panics-in-rust/)

---

## 4. Module Organization

### `lib.rs` + `main.rs` — The Canonical Split

The most important structural improvement for a CLI codebase is extracting all logic into a library crate (`src/lib.rs`), leaving `main.rs` as a thin entry point. This enables integration testing without spawning a subprocess, makes internal functions accessible to tests, and provides clean separation of concerns.

**phyllotaxis current layout:**
```
src/
├── main.rs          ← entry point + dispatch logic
├── spec.rs          ← spec loading
├── commands/        ← command implementations
├── models/          ← data models
└── render/          ← output formatting
```

**Recommended layout:**
```
src/
├── main.rs          ← only: parse CLI args, call lib, handle errors
├── lib.rs           ← re-exports all public surface: commands, models, render, spec
├── spec.rs
├── commands/
│   ├── mod.rs
│   ├── overview.rs
│   └── ...
├── models/
└── render/
```

### Module File Conventions

Modern Rust (since edition 2018) prefers `foo.rs` + `foo/bar.rs` over `foo/mod.rs` + `foo/bar.rs`. The `mod.rs` convention still works but is considered legacy — it creates confusing editor tabs when multiple files are named `mod.rs`. The phyllotaxis codebase already correctly uses `commands/mod.rs` style; migrating to named files (`commands.rs` + `commands/` directory) would be idiomatic but is not urgent.

### Visibility

Default to private. Only expose via `pub` what is needed by other modules or the binary. Use `pub(crate)` for items that should be visible within the crate but not to external callers:

```rust
pub(crate) fn internal_helper() { ... }
```

**Sources:**
- [Separating Modules into Different Files — The Rust Programming Language](https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html)
- [Rust Module and Crate Organization Best Practices](https://softwarepatternslexicon.com/patterns-rust/5/11/)
- [Module/mod.rs or module.rs? — Rust Forum](https://users.rust-lang.org/t/module-mod-rs-or-module-rs/122653)

---

## 5. Clippy and Lints

### Setting Up Clippy

Run Clippy regularly and keep the codebase warning-free. A clean Clippy baseline means new warnings are always visible, not buried in noise:

```sh
cargo clippy -- -D warnings
```

Add to CI to block merges with new warnings.

### What to Enable

**Start with:** `cargo clippy` — this enables the `clippy::correctness`, `clippy::suspicious`, `clippy::complexity`, `clippy::perf`, and `clippy::style` groups by default. These are well-calibrated and rarely generate false positives.

**Consider enabling `clippy::pedantic`** selectively. Enable the whole group, then `#[allow]` the lints that are too noisy for your codebase. Pedantic lints catch real issues (e.g., `clippy::must_use_candidate`, `clippy::missing_errors_doc`).

**Do not enable `clippy::restriction` as a whole** — it contains lints that actively contradict each other and conflict with idiomatic Rust.

### In-Crate Lint Configuration

Place at the top of `main.rs` or `lib.rs`:

```rust
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)] // common false positive
```

Or configure per-lint via `Cargo.toml` (supported since Rust 1.73):

```toml
[lints.clippy]
pedantic = "warn"
module_name_repetitions = "allow"
```

### High-Value Individual Lints

| Lint | What It Catches |
|---|---|
| `clippy::unwrap_used` | All `unwrap()` calls |
| `clippy::expect_used` | All `expect()` calls (strict) |
| `clippy::string_to_string` | Unnecessary `.to_string()` on `String` |
| `clippy::needless_pass_by_value` | `&T` preferred over `T` in params |
| `clippy::inefficient_to_string` | Slower `.to_string()` where `.as_str()` suffices |
| `clippy::cloned_instead_of_copied` | `.cloned()` where `.copied()` works |
| `clippy::redundant_clone` | Clones that the borrow checker doesn't require |

**Sources:**
- [Clippy's Lints — Official Documentation](https://doc.rust-lang.org/stable/clippy/lints.html)
- [Item 29: Listen to Clippy — Effective Rust](https://effective-rust.com/clippy.html)
- [Linting in Rust with Clippy — LogRocket Blog](https://blog.logrocket.com/rust-linting-clippy/)

---

## 6. Testing

### Structure

Keep unit tests co-located with the code they test using `#[cfg(test)]` modules. This allows testing private functions directly:

```rust
// In commands/resources.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_normalization() { ... }
}
```

Put integration tests in `tests/` — these compile as separate crates and can only access public APIs. They are the right place for end-to-end tests of CLI behavior.

### `assert_cmd` for CLI Integration Testing

`assert_cmd` makes it easy to test the compiled binary as a black box:

```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_overview_text_output() {
    Command::cargo_bin("phyllotaxis")
        .unwrap()
        .arg("--spec")
        .arg("tests/fixtures/petstore.yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains("OpenAPI"));
}
```

### `insta` for Snapshot Testing

Snapshot testing is particularly well-suited for CLI tools that produce structured text or JSON output. Rather than hardcoding expected strings, `insta` stores approved snapshots and highlights diffs when output changes:

```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
```

```rust
#[test]
fn test_schema_output() {
    let output = render_schema_detail(&model);
    insta::assert_snapshot!(output);
}
```

Workflow: run tests once (new snapshot files are created as `.pending`), review with `cargo insta review`, then accept. Subsequent runs fail if output changes unexpectedly.

`insta_cmd` provides the combination of `assert_cmd` + snapshot testing:

```rust
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};

#[test]
fn test_resources_json() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("phyllotaxis"))
        .args(["--json", "resources"]));
}
```

### Testing IO-Independent Logic

For command logic that doesn't require a subprocess, the `lib.rs` split (see Section 4) allows direct function-level testing:

```rust
#[test]
fn test_extract_resource_groups() {
    let api = load_test_api("tests/fixtures/petstore.yaml");
    let groups = phyllotaxis::commands::resources::extract_resource_groups(&api);
    assert_eq!(groups.len(), 3);
}
```

**Sources:**
- [Testing — Command Line Applications in Rust](https://rust-cli.github.io/book/tutorial/testing.html)
- [How I test Rust command-line apps with assert_cmd](https://alexwlchan.net/2025/testing-rust-cli-apps-with-assert-cmd/)
- [Using Insta for Rust snapshot testing — LogRocket](https://blog.logrocket.com/using-insta-rust-snapshot-testing/)
- [Testing CLIs — Insta Docs](https://insta.rs/docs/cmd/)

---

## 7. Performance and Allocation

### Key Principles

For a CLI tool like phyllotaxis, startup time and memory pressure during a single invocation are the main concerns. The principles below apply at the point where they matter — don't prematurely optimize, but do follow these patterns habitually:

**Prefer borrowed types in function signatures:**

```rust
// Instead of:
fn process(name: String) -> String

// Prefer:
fn process(name: &str) -> String   // or &str if output borrows input
```

This is `clippy::needless_pass_by_value` — it eliminates a clone at every call site.

**Use `Cow<'a, str>` for values that are sometimes borrowed and sometimes owned:**

```rust
use std::borrow::Cow;

fn normalize(name: &str) -> Cow<str> {
    if name.chars().all(|c| c.is_lowercase()) {
        Cow::Borrowed(name)   // no allocation
    } else {
        Cow::Owned(name.to_lowercase())  // allocate only when needed
    }
}
```

**Pre-allocate collections when the size is known:**

```rust
let mut results = Vec::with_capacity(endpoints.len());
```

**Avoid `format!` for string assembly when a literal suffices.** Every `format!` call allocates. For fixed strings, use string literals. For simple concatenation of a few parts, consider `String::new()` + `.push_str()` in a tight loop.

**Reuse buffers in loops:**

```rust
let mut line = String::new();
while reader.read_line(&mut line)? > 0 {
    process(&line);
    line.clear();  // reuses allocation
}
```

**Avoid intermediate `collect()` calls.** If you chain iterator operations and the result feeds directly into another iterator, omit the `collect()`:

```rust
// Unnecessary collect:
let filtered: Vec<_> = items.iter().filter(|x| x.valid).collect();
filtered.iter().map(|x| x.name).for_each(|n| println!("{}", n));

// Better: keep it lazy
items.iter()
    .filter(|x| x.valid)
    .map(|x| x.name.as_str())
    .for_each(|n| println!("{}", n));
```

**Sources:**
- [Heap Allocations — The Rust Performance Book](https://nnethercote.github.io/perf-book/heap-allocations.html)
- [Performance Considerations: When to Use `Cow<str>` — Sling Academy](https://www.slingacademy.com/article/performance-considerations-when-to-use-cow-str-in-rust/)
- [Zero-Copy in Rust: Challenges and Solutions](https://coinsbench.com/zero-copy-in-rust-challenges-and-solutions-c0d38a6468e9)

---

## 8. Type Design

### Newtype Pattern

Replace bare primitives with domain-meaningful types to catch entire classes of bugs at compile time. There is no runtime cost — the wrapper is optimized away:

```rust
// Instead of: fn find_resource(name: &str, method: &str, path: &str)
// Errors are easy: find_resource("GET", "/pets", "pets")

struct ResourceName(String);
struct HttpMethod(String);
struct EndpointPath(String);

fn find_resource(name: &ResourceName, method: &HttpMethod, path: &EndpointPath) { ... }
```

The compiler now prevents argument order mistakes. Implement `FromStr`, `Display`, `AsRef<str>`, and `Deref<Target = str>` to keep usage ergonomic.

### Avoid Stringly-Typed Code

The Rust API Guidelines ([C-CUSTOM-TYPE](https://rust-lang.github.io/api-guidelines/type-safety.html)) recommend avoiding bare primitives like `bool`, `u8`, and `String` as function arguments when custom types would communicate intent more clearly.

For phyllotaxis, `method: Option<String>` in the `Resources` command struct could become `method: Option<HttpMethod>` to make the accepted values explicit. At minimum, validate at parse time rather than at use time.

### Builder Pattern

For structs with many optional fields, the builder pattern reduces constructor complexity. Use non-consuming builders (take `&mut self`, return `&mut Self`) for ergonomic chaining:

```rust
EndpointDetailBuilder::new("GET", "/pets")
    .with_expansion(true)
    .with_depth(5)
    .build()?
```

The Rust API Guidelines ([C-BUILDER](https://rust-lang.github.io/api-guidelines/type-safety.html)) recommend this pattern when construction has meaningful optional configuration.

### "Parse, Don't Validate"

A powerful idiom: validate data once at the boundary and encode the result in a type. Downstream code receives a type that _proves_ the data is valid:

```rust
struct ValidatedSpecPath(PathBuf);  // can only be constructed by path_exists()

fn load_spec_path(raw: &str) -> Result<ValidatedSpecPath> {
    let path = PathBuf::from(raw);
    if !path.exists() {
        return Err(anyhow!("Spec file not found: {}", raw));
    }
    Ok(ValidatedSpecPath(path))
}
```

**Sources:**
- [Newtype — Rust Design Patterns](https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html)
- [Type Safety — Rust API Guidelines](https://rust-lang.github.io/api-guidelines/type-safety.html)
- [The Ultimate Guide to Rust Newtypes](https://www.howtocodeit.com/guides/ultimate-guide-rust-newtypes)

---

## 9. Lifetime and Ownership Patterns

### Default to Immutable Borrows

For a CLI tool with a linear data flow (parse args → load spec → build model → render output), most functions should accept `&T` references. The spec is loaded once and read-only throughout a single invocation; there is no need for ownership at most function boundaries.

```rust
// Don't require ownership if you only need to read:
fn render_overview(data: &OverviewData) -> String { ... }

// The caller retains ownership; the data lives for the whole invocation
```

### Avoid Cloning to Satisfy the Borrow Checker

When the borrow checker complains, the fix is usually a design issue, not a `clone()`. Cloning to satisfy the borrow checker (`clone_to_satisfy_the_borrow_checker` is a named anti-pattern in the Rust design patterns book) is the sign that the ownership model needs rethinking.

Common cases where clone is unnecessary:
- You need a value in two places — pass a reference to both instead
- A function takes `String` when it only needs `&str`
- A struct holds owned data that it only needs to read

`cargo clippy` catches many of these with `clippy::redundant_clone` and `clippy::needless_pass_by_value`.

### When to Move vs. Borrow

| Situation | Pattern |
|---|---|
| Read-only access, single invocation | `&T` reference |
| Transforming a value into a new form | Move (consume the input) |
| Long-lived shared access | `Rc<T>` (single thread) or `Arc<T>` (multi-thread) |
| Primitive types (`u32`, `bool`, `char`) | Pass by value (`Copy` is free) |

### Avoid Unnecessary Lifetime Annotations on Structs

Lifetime annotations on structs create significant API complexity. Avoid storing references in structs unless the performance gain is measured and critical. Owned data is simpler and usually fast enough:

```rust
// Avoid this unless you have a measured reason:
struct Renderer<'a> {
    spec: &'a OpenApi,
}

// Prefer:
struct Renderer {
    spec: Arc<OpenApi>,  // shared ownership, no lifetime complexity
}
```

**Sources:**
- [Clone to satisfy the borrow checker — Rust Design Patterns (Anti-pattern)](https://rust-unofficial.github.io/patterns/anti_patterns/borrow_clone.html)
- [Rust Patterns for Lifetime Management](https://sigwait.com/rust-patterns-for-lifetime-management)

---

## 10. Documentation

### What to Document

Public API items should have `///` doc comments. Apply `#![warn(missing_docs)]` in `lib.rs` to make missing documentation a compile warning. For a CLI tool with a `lib.rs` surface, document all public functions, types, and modules.

### Standard Sections

The [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html) and [RFC 1574](https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html) define standard doc comment sections:

- **Summary line** — the first line; appears in search results and module overviews. Keep it to one sentence.
- **`# Panics`** — any conditions under which the function panics
- **`# Errors`** — for `Result`-returning functions, what errors can be returned
- **`# Examples`** — use the plural form always; rustdoc will compile and test these

```rust
/// Resolves the spec file path using the configured priority chain.
///
/// Searches in order: `--spec` flag, config file `specs` map, `default` key,
/// legacy `spec` field, then auto-detection in the current directory.
///
/// # Errors
///
/// Returns an error if no spec file can be located or if the resolved path
/// does not exist.
///
/// # Examples
///
/// ```no_run
/// let path = resolve_spec_path(None, &None, Path::new("."))?;
/// ```
pub fn resolve_spec_path(...) -> Result<PathBuf> { ... }
```

### Module-Level Docs

Use `//!` at the top of each module file to describe the module's purpose. This appears on the module's rustdoc page.

### Doc Tests

Code examples in `///` comments are compiled and run by `cargo test`. This keeps examples accurate. Use `# ` (hash + space) to prefix setup lines that should compile but not appear in rendered docs:

```rust
/// ```
/// # use phyllotaxis::models::resource::ResourceGroup;
/// let group = ResourceGroup::new("pets");
/// assert_eq!(group.name(), "pets");
/// ```
```

**Sources:**
- [Documentation — Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
- [How to write documentation — The rustdoc book](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
- [RFC 1574 — More API Documentation Conventions](https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html)

---

## 11. Edition 2021 Features

Phyllotaxis uses Edition 2021 (correct). Here is what Edition 2021 provides that should be actively used:

### Disjoint Closure Captures

Closures now capture individual struct fields rather than the whole struct. This often eliminates the need for artificial variable bindings when moving data into closures:

```rust
struct Config { path: PathBuf, name: String }
let config = Config { ... };
let name = config.name;  // Edition 2018: had to do this
let f = move || use(config.path);  // Edition 2021: just works — captures only `path`
```

### `IntoIterator` for Arrays

Arrays now implement `IntoIterator` by value. `array.into_iter()` now yields owned `T`, not `&T`:

```rust
let arr = [1u32, 2, 3];
for x in arr { ... }  // x: u32, not &u32
```

### `TryFrom` / `TryInto` in Prelude

`TryFrom`, `TryInto`, and `FromIterator` are now in the prelude — no explicit `use std::convert::TryFrom` required. This makes newtype implementations cleaner.

### Cargo Feature Resolver v2 (Default)

The feature resolver 2 is the default in Edition 2021 workspaces. It avoids unifying dev-dependency features with production dependency features — important for keeping test-only dependencies out of release builds.

### `rust-version` in `Cargo.toml`

Specify the minimum supported Rust version:

```toml
[package]
rust-version = "1.75"
```

Cargo will error early with a clear message if the toolchain is too old.

**Sources:**
- [Rust 2021 — The Rust Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2021/index.html)
- [Announcing Rust 1.56.0 and Rust 2021 — Rust Blog](https://blog.rust-lang.org/2021/10/21/Rust-1.56.0/)

---

## 12. Cargo Best Practices

### Release Profile

Phyllotaxis has no `[profile.release]` section in `Cargo.toml`. The defaults are reasonable but not optimal for a distributed CLI binary. Recommended production settings:

```toml
[profile.release]
opt-level = 3       # default; maximize speed
lto = true          # link-time optimization — smaller binary, faster code
codegen-units = 1   # disable parallel codegen for better optimization
panic = "abort"     # smaller binary; panics terminate immediately without stack unwinding
strip = true        # strip debug symbols from release binary
```

**Trade-offs:**
- `lto = true` and `codegen-units = 1` significantly increase compile time for release builds
- `panic = "abort"` prevents using `std::panic::catch_unwind`; acceptable for a CLI tool
- `strip = true` requires Rust 1.59+; eliminates debug symbol overhead in distributed binaries

### Dev Profile

For faster iteration during development, the defaults are fine. If compile times are a pain point:

```toml
[profile.dev]
debug = 1   # reduce debug info for faster compile (0=none, 1=line tables only, 2=full)
```

### Feature Flags

Use feature flags to gate optional dependencies and functionality. For phyllotaxis, a future `--json` output dependency on `serde_json` could be feature-gated if text-only builds are desired.

### `rust-analyzer` Configuration

Keep `.cargo/config.toml` or `rust-analyzer` settings tuned for fast feedback. Add `Cargo.lock` to version control for binary crates (it ensures reproducible builds).

**Sources:**
- [Profiles — The Cargo Book](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Build Configuration — The Rust Performance Book](https://nnethercote.github.io/perf-book/build-configuration.html)
- [Customizing Builds with Release Profiles — The Rust Programming Language](https://doc.rust-lang.org/book/ch14-01-release-profiles.html)

---

## 13. CLI-Specific Patterns: Separating IO from Logic

### The Core Problem

CLI tools that mix I/O (reading files, writing stdout, reading env vars) into their business logic are hard to test. Every test requires spawning a subprocess or mocking the filesystem.

### "Functional Core, Imperative Shell"

The practical version of hexagonal architecture for CLI tools. Principle: push all I/O to the edges, keep the core logic pure:

```
main()           ← parse args, call IO, call core, print result, handle errors
  │
  ├─ load_spec() ← IO: reads file, parses YAML
  │
  └─ commands::resources::extract_resource_groups()  ← pure: transforms data
       │
       └─ render::text::render_resource_list()  ← pure: formats data to String
```

The pure functions (`extract_resource_groups`, `render_resource_list`) can be unit-tested without touching the filesystem. The IO functions (`load_spec`) need integration tests or filesystem fixtures.

### The `std::io::Write` Abstraction

Rather than writing directly to `stdout`, accept a writer parameter. This makes rendering functions testable without capturing stdout:

```rust
fn render_overview(data: &OverviewData, writer: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(writer, "API: {}", data.title)?;
    writeln!(writer, "Version: {}", data.version)?;
    Ok(())
}

// In tests:
let mut output = Vec::new();
render_overview(&data, &mut output).unwrap();
assert!(String::from_utf8(output).unwrap().contains("API:"));

// In main:
render_overview(&data, &mut std::io::stdout())?;
```

This pattern is described in the [Command Line Applications in Rust book](https://rust-cli.github.io/book/tutorial/testing.html) as the primary technique for making CLI output testable.

### Exit Codes

The current `die()` function always exits with code 1. For machine-readable CLIs, distinct exit codes communicate different failure modes. The standard conventions:

- `0` — success
- `1` — general error (operational failure)
- `2` — usage error (bad arguments)
- `127` — command not found (shell convention)

For structured exit code management, consider the `std::process::ExitCode` type (stabilized in Rust 1.61) or the `exitcode` crate which provides named constants.

### Hexagonal Architecture (Ports and Adapters)

For larger CLIs, define traits for external interactions and swap implementations in tests:

```rust
trait SpecLoader {
    fn load(&self, path: &Path) -> Result<OpenApi>;
}

struct FileSpecLoader;
impl SpecLoader for FileSpecLoader {
    fn load(&self, path: &Path) -> Result<OpenApi> { ... }
}

// In tests:
struct MockSpecLoader { content: OpenApi }
impl SpecLoader for MockSpecLoader {
    fn load(&self, _path: &Path) -> Result<OpenApi> { Ok(self.content.clone()) }
}
```

This level of abstraction is more complexity than phyllotaxis currently needs, but the principle — **keep core logic independent of IO** — applies even without traits.

**Sources:**
- [Testing — Command Line Applications in Rust](https://rust-cli.github.io/book/tutorial/testing.html)
- [Master Hexagonal Architecture in Rust](https://www.howtocodeit.com/articles/master-hexagonal-architecture-rust)
- [Hexagonal Architecture in Rust — Cogs and Levers](http://tuttlem.github.io/2025/08/31/hexagonal-architecture-in-rust.html)
- [Building CLI Apps in Rust — What You Should Consider](https://betterprogramming.pub/building-cli-apps-in-rust-what-you-should-consider-99cdcc67710c)

---

## Summary: Priority Recommendations for phyllotaxis

Based on this research, the highest-priority improvements for phyllotaxis are:

| Priority | Change | Why |
|---|---|---|
| High | Add `anyhow`; replace `Result<_, String>` | Better error messages, ergonomic propagation, display chain |
| High | Add `thiserror` for typed spec errors | Enables matching on "not found" vs. "parse error" in tests |
| High | Extract `src/lib.rs` | Unlocks unit testing of internal logic without subprocess |
| High | Add `assert_cmd` + `insta` dev deps | Modern, low-maintenance integration and snapshot testing |
| Medium | Add `[profile.release]` with `lto`, `codegen-units = 1` | Better release binary performance |
| Medium | Run `cargo clippy -- -D warnings` and address all warnings | Catch idiomatic issues; normalize style |
| Medium | Replace `&String` params with `&str` | Avoids unnecessary constraint on callers |
| Low | Add `#![warn(missing_docs)]` to `lib.rs` | Enforce documentation coverage incrementally |
| Low | Consider `Cow<str>` in render functions | Avoids allocations when strings need no modification |
| Low | Add `rust-version` to `Cargo.toml` | Documents minimum supported Rust version |
