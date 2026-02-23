# Rust CLI Security Best Practices: Research Report

**Project context:** phyllotaxis — a Rust CLI tool for progressive disclosure of OpenAPI specs
**Prepared:** 2026-02-21
**Scope:** Security best practices for Rust CLI applications, with specific applicability to phyllotaxis

---

## Table of Contents

1. [Dependency Security](#1-dependency-security)
2. [Input Validation and Path Handling](#2-input-validation-and-path-handling)
3. [Error Handling Security](#3-error-handling-security)
4. [Secrets and Sensitive Data](#4-secrets-and-sensitive-data)
5. [File System Operations](#5-file-system-operations)
6. [Output Security](#6-output-security)
7. [Cargo.toml Best Practices](#7-cargotoml-best-practices)
8. [CI/CD Security Practices](#8-cicd-security-practices)
9. [Known Vulnerability Patterns in the Rust Ecosystem](#9-known-vulnerability-patterns-in-the-rust-ecosystem)
10. [Phyllotaxis-Specific Findings](#10-phyllotaxis-specific-findings)
11. [Sources](#11-sources)

---

## 1. Dependency Security

### The Supply Chain Problem

Rust's ecosystem depends on crates.io, and as of 2025 the ecosystem is large enough to attract supply chain attacks. Typosquatting, compromised maintainer accounts, and unmaintained crates with unpatched vulnerabilities are the primary threat vectors. Unlike npm's JavaScript ecosystem (which has had thousands of malicious packages), Rust's supply chain is comparatively cleaner, but the risk is real and growing.

### cargo-audit

`cargo-audit` is the baseline tool for dependency security. It audits `Cargo.lock` against the [RustSec Advisory Database](https://rustsec.org/), which tracks known CVEs and security advisories for Rust crates.

```bash
cargo install cargo-audit --locked
cargo audit
```

Key capabilities:
- Detects crates with known vulnerabilities by cross-referencing `Cargo.lock` against RustSec
- Flags unmaintained crates (which accumulate unpatched vulnerabilities)
- Has an experimental `--fix` mode to auto-upgrade vulnerable dependencies
- Can audit compiled binaries if built with `cargo-auditable`

**Phyllotaxis relevance:** `serde_yaml 0.9` is the version in use. The original `serde_yaml` crate was deprecated by its author (dtolnay) in mid-2024. There is no formal RustSec advisory filed against the `serde_yaml` 0.9.x line, but the `serde_yml` fork (a popular migration target) received **RUSTSEC-2025-0068** for unsoundness: `serde_yml::ser::Serializer.emitter` can cause a segmentation fault. The maintained forks are `serde_yaml_ng` and `serde_norway`. This is an active area of risk worth tracking.

### cargo-deny

`cargo-deny` goes further than `cargo-audit`. Configured via a `deny.toml` file, it checks:

- **Advisories:** security vulnerabilities from RustSec (same DB as cargo-audit)
- **Licenses:** rejects dependencies whose licenses conflict with your policy
- **Bans:** explicitly disallows specific crates, or crates with multiple conflicting versions
- **Sources:** ensures all crates come from trusted sources (e.g., no git dependencies from random repos)

```toml
# deny.toml example
[licenses]
allow = ["MIT", "Apache-2.0", "ISC", "BSD-3-Clause"]

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"

[bans]
multiple-versions = "warn"
wildcards = "deny"
```

`cargo-deny` is more opinionated and requires upfront investment in configuration, but it is the right tool for a project that wants a comprehensive gate. The `cargo-deny-action` GitHub Action makes this trivial to run in CI.

### cargo-geiger: Unsafe Code Detection

`cargo-geiger` scans your dependency tree for unsafe code usage:

```bash
cargo install cargo-geiger
cargo geiger
```

Output marks each crate with `:)` (no unsafe, explicitly forbids it), `?` (no unsafe found), or `!` (unsafe code present). This helps you identify high-risk dependencies that could harbor memory safety issues the borrow checker cannot prevent.

### cargo-auditable: Embedding Dependency Data in Binaries

`cargo-auditable` embeds the full dependency tree (as JSON) into a dedicated linker section of compiled binaries. This makes production binaries auditable long after they're built, without needing the original `Cargo.lock`:

```bash
cargo install cargo-auditable
cargo auditable build --release
cargo audit bin ./target/release/phyllotaxis
```

Alpine Linux, NixOS, openSUSE, and Void Linux all build their Rust packages with `cargo-auditable`.

---

## 2. Input Validation and Path Handling

### Path Traversal (CWE-22)

Path traversal is among the most common vulnerabilities in Rust CLIs that read files. The attack involves supplying a path like `../../../../etc/passwd` to escape an intended directory. The `PathBuf::join()` method resolves `../` sequences automatically, so naively joining user input to a base directory is not safe:

```rust
// UNSAFE — user can traverse out of base_dir
let path = base_dir.join(user_input);
```

The correct mitigation is to canonicalize the resolved path and verify it is still within the intended base:

```rust
use std::fs;
use std::path::Path;

fn safe_path(base: &Path, user_input: &str) -> Result<PathBuf, &'static str> {
    let joined = base.join(user_input);
    let canonical = fs::canonicalize(&joined).map_err(|_| "invalid path")?;
    if canonical.starts_with(base) {
        Ok(canonical)
    } else {
        Err("path traversal detected")
    }
}
```

Two real-world Rust CVEs make this concrete:
- **CVE-2021-45712** — path traversal in `rust-embed`
- **CVE-2025-68705** — path traversal in `rustfs` (CVSS ~9.9) — root cause was no `canonicalize()` call before validation, and no boundary check

**Phyllotaxis relevance:** phyllotaxis reads OpenAPI files from disk using user-supplied paths. It also writes a `.phyllotaxis.yaml` config file via `init`. Both are path traversal surfaces. The write path for `init` deserves particular scrutiny — writing to an attacker-controlled path could overwrite sensitive files.

### clap Input Validation

`clap`'s derive API supports custom value parsers via `#[arg(value_parser)]`. This is the right place to validate file path arguments before they reach file I/O:

```rust
#[derive(Parser)]
struct Cli {
    #[arg(value_parser = validate_openapi_path)]
    spec: PathBuf,
}

fn validate_openapi_path(s: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(s);
    if !p.exists() {
        return Err(format!("file not found: {}", s));
    }
    // Additional checks: extension, size, etc.
    Ok(p)
}
```

For phyllotaxis, the spec file path is user-supplied. Validation at the `clap` layer (before any parsing happens) is safer than validating after the fact.

### YAML/JSON Deserialization Risks

`serde` is intentionally data-only — it does not execute code during deserialization, which eliminates the class of RCE vulnerabilities common in Java/Python deserializers. However, two risks remain:

1. **Stack overflow via deeply nested structures.** A maliciously crafted YAML file with thousands of nested mappings can overflow the stack during recursive deserialization. This is a denial-of-service vector. Mitigation: validate file size before parsing, and consider a recursion depth limit.

2. **Logic errors from malformed data.** Accepting invalid data is a security risk. Always validate deserialized structs against your expected schema rather than trusting the parsed output blindly.

---

## 3. Error Handling Security

### Information Leakage

Error messages are a common information leakage channel. Exposing internal paths, stack traces, or implementation details helps attackers understand your system. For a CLI tool, this manifests when error messages include:

- Full filesystem paths (e.g., `/home/username/.config/...`)
- Internal crate/function names
- Deserialization details that reveal data structure internals

**Best practice:** Use a two-layer error model. Internal errors carry full context for logging/debugging; user-facing errors are sanitized:

```rust
// Internal: rich detail for developers
#[derive(Debug)]
enum AppError {
    YamlParse(serde_yaml::Error),
    FileNotFound(PathBuf),
    InvalidSpec(String),
}

// User-facing: sanitized, no internal paths
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::YamlParse(_) => write!(f, "failed to parse OpenAPI spec: invalid YAML"),
            AppError::FileNotFound(p) => write!(f, "spec file not found: {}", p.display()),
            AppError::InvalidSpec(msg) => write!(f, "invalid OpenAPI spec: {}", msg),
        }
    }
}
```

Notice that `YamlParse` does not expose the underlying `serde_yaml::Error` (which may include internal parser state). `FileNotFound` does expose the path — which is appropriate since the user supplied it — but this deserves deliberate consideration.

### Avoiding `.unwrap()` in Production Code

`.unwrap()` panics on error, producing a Rust backtrace that includes function names, file paths, and line numbers — all potentially useful to attackers. Use `?` propagation or explicit error handling instead:

```rust
// BAD: panics with backtrace on failure
let spec = fs::read_to_string(path).unwrap();

// GOOD: propagates error through Result
let spec = fs::read_to_string(path)?;
```

---

## 4. Secrets and Sensitive Data

### The Problem

phyllotaxis has an `auth` command, which implies it may handle API keys, tokens, or credentials at some point. Rust's memory model does not automatically zero sensitive data when it goes out of scope — the `Drop` impl for `String` and `Vec` frees memory but does not zero it first. That memory may be read from swap, a core dump, or a process memory inspector before the OS reclaims it.

Additionally, the compiler may optimize away zeroing operations it considers "dead stores" unless specific care is taken.

### The zeroize Crate

[`zeroize`](https://docs.rs/zeroize/latest/zeroize/) is the standard solution. It uses volatile semantics and compiler fences to ensure the zeroing cannot be elided:

```rust
use zeroize::Zeroize;

let mut secret = String::from("my-api-key");
// ... use secret ...
secret.zeroize(); // guaranteed to zero the backing buffer
```

The `Zeroizing<T>` wrapper automatically zeroes on drop:

```rust
use zeroize::Zeroizing;

let secret = Zeroizing::new(String::from("my-api-key"));
// automatically zeroed when secret goes out of scope
```

**Limitation:** `zeroize` cannot protect against microarchitectural attacks (Spectre/Meltdown) that leak via cache side channels. It also cannot guarantee that prior buffer reallocations did not leave copies in memory.

### The secrecy Crate

[`secrecy`](https://docs.rs/secrecy/latest/secrecy/) wraps secrets in types that:
- Zero on drop (via `zeroize`)
- Redact the value in `Debug` output (prints `[REDACTED]` instead of the actual value)
- Require explicit `expose_secret()` calls to access the raw value

```rust
use secrecy::{SecretString, ExposeSecret};

let token = SecretString::new("sk-abc123".into());
println!("{:?}", token); // prints: Secret([REDACTED])
let raw = token.expose_secret(); // explicit access
```

This prevents accidental logging of secrets — a common source of credential leakage in CLI tools that use structured logging.

### Never Log Secrets

This is obvious but frequently violated. Even if a secret is `zeroize`d correctly, logging it first defeats the purpose. If phyllotaxis ever holds API tokens (from config files, env vars, or CLI args):

- Never pass them through format strings in log statements
- Use `secrecy::SecretString` so that `Debug` impls automatically redact
- Never write them to the `.phyllotaxis.yaml` config file in plaintext unless that is an explicit, documented design choice

---

## 5. File System Operations

### TOCTOU Race Conditions

Time-of-Check to Time-of-Use (TOCTOU) vulnerabilities occur when a program checks a property of a file (existence, type, permissions) and then acts on that check, but the filesystem state changes in between. This is a filesystem-level race condition that Rust's memory safety does not prevent.

A real-world example: **CVE-2022-21658** was a TOCTOU vulnerability in Rust's own standard library `std::fs::remove_dir_all`. An attacker could race between the symlink check and the recursive delete to cause the function to follow symlinks and delete unintended files. Rust 1.0.0 through 1.58.0 were affected.

The general principle: **do not check-then-act on filesystem state.** Instead, open a file descriptor and operate on it atomically, or use error handling rather than pre-checking:

```rust
// BAD: check then act (TOCTOU window)
if path.exists() {
    fs::read_to_string(path)?;
}

// GOOD: act and handle the error
match fs::read_to_string(path) {
    Ok(content) => { /* use content */ }
    Err(e) if e.kind() == io::ErrorKind::NotFound => { /* handle missing */ }
    Err(e) => return Err(e.into()),
}
```

### Config File Write Safety (init command)

phyllotaxis's `init` command writes a `.phyllotaxis.yaml` file. Safe file writing should:

1. Write to a temporary file first, then rename atomically (rename is atomic on POSIX systems)
2. Not overwrite an existing file without confirmation (to prevent accidental data loss)
3. Validate the output path is within the expected directory

```rust
// Atomic write pattern
let tmp = path.with_extension("yaml.tmp");
fs::write(&tmp, content)?;
fs::rename(&tmp, &path)?; // atomic on POSIX
```

### Symlink Following

When reading the OpenAPI spec file, if phyllotaxis follows symlinks, an attacker with write access to the symlink target location could redirect the read to a sensitive file. For a CLI tool reading user-supplied paths this is usually acceptable (the user controls the path), but it is worth documenting and considering in multi-user environments.

---

## 6. Output Security

### ANSI Escape Code Injection (Terminal Injection)

When a CLI outputs data from an untrusted source (like field values from a parsed OpenAPI spec), that data may contain ANSI escape sequences. If printed directly to the terminal, these sequences can:

- Change terminal window titles
- Rewrite previous terminal output (spoofing)
- Cause denial of service by flooding output
- In historical cases, enable arbitrary code execution via terminal input injection

This is a real, documented attack class. **CVE-2021-25743** demonstrated ANSI injection in Kubernetes and OpenShift. The `tracing-subscriber` Rust crate had a confirmed ANSI injection vulnerability (reported May 2025).

**For phyllotaxis:** the `resources`, `schemas`, `search`, and `auth` commands output data from user-controlled OpenAPI spec files. An API spec could contain crafted strings in field names, descriptions, or paths. If those strings contain ANSI escape codes and phyllotaxis prints them to the terminal, the terminal can be manipulated.

Mitigation options:

1. **Strip ANSI codes from output when writing to a terminal.** Check `std::io::IsTerminal` and sanitize if true:

```rust
use std::io::IsTerminal;

fn sanitize_for_terminal(s: &str) -> String {
    // Strip ANSI escape sequences: \x1b[ ... m and similar
    // The regex crate or a simple state machine can do this
    s.chars()
        .filter(|&c| c != '\x1b')
        .collect()
}

if std::io::stdout().is_terminal() {
    println!("{}", sanitize_for_terminal(&value));
} else {
    println!("{}", value); // piped output: consumer handles it
}
```

2. **Use a dedicated sanitization crate.** The `strip-ansi-escapes` crate (crates.io) provides a byte-level filter.

3. **Distinguish JSON vs. text output.** When outputting JSON (machine-readable), ANSI injection is not a terminal risk because the terminal does not interpret JSON values. The risk is specific to text output rendered directly.

### JSON Output Safety

When phyllotaxis outputs JSON, it should always do so via `serde_json::to_string()` on a typed struct — never by hand-constructing JSON strings. This guarantees proper escaping of all values:

```rust
// SAFE: serde_json escapes all values properly
let output = serde_json::to_string(&my_struct)?;

// UNSAFE: manual construction can produce malformed or injected JSON
let output = format!("{{\"name\": \"{}\"}}", user_value); // user_value may contain "
```

`serde_json` handles Unicode, special characters, and control characters correctly when serializing typed Rust structs.

---

## 7. Cargo.toml Best Practices

### Version Pinning Strategy

phyllotaxis's current `Cargo.toml` uses range version specifiers (e.g., `clap = { version = "4" }`). This is standard Rust practice — Cargo's SemVer resolution is well-designed and `Cargo.lock` pins exact versions for reproducible builds. The key security practice is to commit `Cargo.lock` to version control (appropriate for binary crates, which phyllotaxis is).

### Security-Relevant Profile Settings

The release profile has security-relevant defaults worth overriding:

```toml
[profile.release]
# Panic = abort reduces binary size and eliminates unwinding attack surface.
# However, it means Drop impls don't run on panic — consider this if using zeroize.
# panic = "abort"

# Overflow checks catch integer arithmetic bugs that can lead to logic vulnerabilities.
# Default in release is false. Enable explicitly:
overflow-checks = true

# Strip debug info from release binaries to reduce information exposure.
# Keep symbols in a separate file for crash analysis.
strip = "debuginfo"

# LTO reduces binary size and eliminates dead code, reducing attack surface:
lto = true
```

**Key trade-off:** `panic = "abort"` prevents stack unwinding, which means `Drop` impls (including `zeroize`) do not run on panic. If you add secret-handling with `Zeroizing<T>`, keep `panic = "unwind"` or accept that panics may leave secrets in memory briefly.

**overflow-checks:** Integer overflow in release builds silently wraps by default. This can cause logic errors in bounds checking, length calculations, or index math. Enabling `overflow-checks = true` in release makes overflows panic rather than wrap silently. The performance cost is typically negligible for a CLI tool.

**strip = "debuginfo":** Release binaries contain debug info by default (since Rust 1.77, stdlib debug info is stripped automatically, but your code's debug info is not). Stripping reduces the information available to reverse engineers and reduces binary size.

### Dependency Locking

```toml
# Use exact version where stability is critical
indexmap = "=2.13.0"  # Locked to exact version

# Or rely on Cargo.lock for pinning (standard approach)
indexmap = "2"  # SemVer compatible, Cargo.lock pins exact version
```

For a CLI tool distributed to users, committing `Cargo.lock` is correct and important. `cargo install` without `--locked` may use different dependency versions than the developer tested.

---

## 8. CI/CD Security Practices

### Automated Vulnerability Auditing

Run `cargo audit` on every CI build and on a scheduled basis. New advisories are published continuously against existing crates — a dependency that was clean last week may have a CVE today.

**GitHub Actions: On-push audit**
```yaml
- name: Security audit
  uses: rustsec/audit-check@v2
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
```

**GitHub Actions: Scheduled daily audit**
```yaml
on:
  schedule:
    - cron: '0 0 * * *'  # midnight UTC daily
  push:
    paths:
      - 'Cargo.toml'
      - 'Cargo.lock'
```

The scheduled audit is critical because it catches new advisories published against your existing (unchanged) dependencies.

### cargo-deny in CI

```yaml
- name: Dependency checks
  uses: EmbarkStudios/cargo-deny-action@v2
  with:
    command: check advisories bans licenses sources
```

`cargo-deny` fails the build on any disallowed license, banned crate, or new security advisory. This is stricter than `cargo-audit` alone and covers license compliance.

### Clippy as a Security Gate

```yaml
- name: Clippy
  run: cargo clippy -- -D warnings
```

`-D warnings` fails the build on any Clippy lint. Clippy catches patterns that can lead to security issues:
- Incorrect bounds checks
- Potential panics
- Redundant clones that may hold sensitive data longer than necessary
- Incorrect use of unsafe patterns

### Rust Sanitizers (Development)

The Rust compiler supports AddressSanitizer, MemorySanitizer, ThreadSanitizer, and LeakSanitizer. These are nightly-only and add runtime overhead, but they catch bugs that neither the type system nor Clippy can detect:

```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu
```

Run sanitizers in CI on nightly as a periodic (not per-commit) check.

---

## 9. Known Vulnerability Patterns in the Rust Ecosystem

The RustSec Advisory Database reveals recurring patterns in Rust crate vulnerabilities:

### Unsound `unsafe` Code

The most common source of real memory safety bugs in Rust crates is incorrect `unsafe` blocks. Common mistakes:

- **Incorrect `repr` assumptions:** RUSTSEC-2024-0347 involved assuming `#[repr(packed)]` guarantees field order; Rust 1.80 changed this behavior, breaking the safety invariant.
- **Padding byte handling:** RUSTSEC-2024-0435 — `transmute_vec_as_bytes` in `fyrox-core` failed to enforce `Pod` trait requirements, causing uninitialized memory reads.
- **Segmentation faults in serializers:** RUSTSEC-2025-0068 — `serde_yml`'s serializer could segfault, violating Rust's safety guarantees.

**Relevance:** phyllotaxis does not use `unsafe` code directly, but its dependencies do (notably the YAML parsing stack). `cargo-geiger` can map this.

### Cryptographic Timing Attacks

Timing-based side channels appear regularly in the RustSec DB:
- Non-constant-time base64 decoding
- Timing variability in `curve25519-dalek` scalar subtraction
- Timing in equality comparisons of secrets

**Relevance:** phyllotaxis's `auth` command likely handles API tokens. If it compares tokens, use constant-time comparison (`subtle` crate) not `==`.

### Path and Config Manipulation for Code Execution

Multiple advisories in 2024 involved path or configuration manipulation leading to arbitrary code execution:
- **RUSTSEC (gix-path):** `gix-path` improperly resolved configuration paths reported by Git
- **RUSTSEC (gix-transport):** malicious usernames in git transport caused indirect code execution

Pattern: any path that passes through external configuration or user input and then feeds into a process execution or file operation is a potential code execution vector.

### Unmaintained Crates

The `unmaintained` category in RustSec is significant — unmaintained crates accumulate unpatched vulnerabilities over time. `cargo-deny` can be configured to warn or deny on unmaintained crates.

---

## 10. Phyllotaxis-Specific Findings

Based on the research above and the project's `Cargo.toml`, the following are the highest-priority concerns:

### High Priority

**1. serde_yaml is deprecated.** `serde_yaml 0.9` is the upstream-abandoned version. While no formal RustSec advisory exists for 0.9.x, the crate is unmaintained. The `serde_yml` fork has a confirmed unsoundness advisory (RUSTSEC-2025-0068). Recommended path: migrate to `serde_yaml_ng` or `serde_norway`.

**2. ANSI escape injection in text output.** phyllotaxis outputs strings from parsed OpenAPI specs (paths, descriptions, tag names, schema names) to stdout. These can contain ANSI escape sequences. Any text-mode output command (`resources`, `schemas`, `auth`, `search`) should sanitize strings before printing to a terminal.

**3. Path validation on spec file input.** The OpenAPI spec file path is user-supplied. While path traversal against a CLI tool where the user is also the file system owner is lower severity than a server-side vulnerability, it is still worth canonicalizing the path and validating it.

### Medium Priority

**4. Config file write atomicity in init.** The `init` command writes `.phyllotaxis.yaml`. This should use an atomic write pattern (write-to-temp, rename) to avoid partial writes on crash.

**5. overflow-checks in release profile.** Currently not set; defaults to `false` in release. Enable `overflow-checks = true` for safety.

**6. No cargo-deny or cargo-audit in CI.** There is no CI configuration visible in the repository. Adding automated dependency auditing is the single highest-leverage security improvement for a project at this stage.

### Lower Priority

**7. JSON output uses serde_json.** The existing use of `serde_json` for JSON output is correct — typed serialization prevents JSON injection. No action needed, just maintain this practice.

**8. clap handles basic arg validation.** clap's type-safe derive API prevents basic argument confusion. Adding file path validators for spec file arguments would be a defensive improvement.

---

## 11. Sources

- [RustSec Advisory Database](https://rustsec.org/) — the authoritative source for Rust crate CVEs
- [RUSTSEC-2025-0068: serde_yml unsound and unmaintained](https://rustsec.org/advisories/RUSTSEC-2025-0068.html)
- [Comparing Rust supply chain safety tools — LogRocket Blog](https://blog.logrocket.com/comparing-rust-supply-chain-safety-tools/)
- [ANSSI Secure Rust Guidelines](https://anssi-fr.github.io/rust-guide/)
- [GitHub: ANSSI-FR/rust-guide](https://github.com/ANSSI-FR/rust-guide)
- [Corgea — Rust Security Best Practices 2025](https://corgea.com/Learn/rust-security-best-practices-2025)
- [Red Hat Developer — Improve basic programming safety with Rust lang](https://developers.redhat.com/articles/2024/05/21/improve-basic-programming-safety-rust-lang)
- [Rust Foundation — Strengthening Rust Security with Alpha-Omega](https://rustfoundation.org/media/strengthening-rust-security-with-alpha-omega-a-progress-update/)
- [StackHawk — Rust Path Traversal Guide](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)
- [GitHub Advisory: rust-embed path traversal CVE-2021-45712](https://github.com/advisories/GHSA-xrg3-hmf3-rvgw)
- [RustFS Path Traversal CVE-2025-68705](https://github.com/rustfs/rustfs/security/advisories/GHSA-pq29-69jg-9mxc)
- [CyberArk — Abusing Terminal Emulators with ANSI Escape Characters](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters)
- [PacketLabs — Weaponizing ANSI Escape Sequences](https://www.packetlabs.net/posts/weaponizing-ansi-escape-sequences/)
- [GlobalSecurityMag — tracing-subscriber ANSI Escape Injection (2025)](https://www.globalsecuritymag.com/vigilance-fr-rust-tracing-subscriber-write-access-via-ansi-escape-sequence.html)
- [zeroize crate docs](https://docs.rs/zeroize/latest/zeroize/)
- [secrecy crate — secure configuration and secrets management](https://leapcell.io/blog/secure-configuration-and-secrets-management-in-rust-with-secrecy-and-environment-variables)
- [Sling Academy — Zeroizing Sensitive Data in Rust Strings](https://www.slingacademy.com/article/zeroizing-sensitive-data-in-rust-strings-for-security/)
- [Rust Forum — Handling sensitive data in memory](https://users.rust-lang.org/t/handling-sensitive-data-in-memory/14388)
- [GitHub Security Advisory — std::fs::remove_dir_all TOCTOU (CVE-2022-21658)](https://github.com/rust-lang/rust/security/advisories/GHSA-r9cc-f5pr-p3j2)
- [NCC Group — Rustproofing Linux Part 2: Race Conditions](https://www.nccgroup.com/research-blog/rustproofing-linux-part-24-race-conditions/)
- [DeepSource — Rust stdlib vulnerability in fs::remove_dir_all](https://deepsource.com/blog/rust-remove-dir-all-vulnerability)
- [RUSTSEC-2023-0018 — remove_dir_all TOCTOU](https://rustsec.org/advisories/RUSTSEC-2023-0018.html)
- [Cargo Book — Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Markaicode — Rust Security Vulnerabilities 2025](https://markaicode.com/rust-security-vulnerabilities-2025-analysis-mitigation/)
- [cargo-audit — crates.io](https://crates.io/crates/cargo-audit)
- [GitHub: rustsec/audit-check GitHub Action](https://github.com/rustsec/audit-check)
- [GitHub: actions-rust-lang/audit](https://github.com/actions-rust-lang/audit)
- [EmbarkStudios/cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
- [cargo-deny GitHub Action](https://github.com/EmbarkStudios/cargo-deny-action)
- [cargo-deny docs — deny.toml configuration](https://embarkstudios.github.io/cargo-deny/checks/cfg.html)
- [GitHub: rust-secure-code/cargo-auditable](https://github.com/rust-secure-code/cargo-auditable)
- [High Assurance Rust — Error Handling](https://highassurance.rs/chp3/rust_6_error.html)
- [oneuptime — How to Secure Rust APIs Against Common Vulnerabilities](https://oneuptime.com/blog/post/2026-01-07-rust-api-security/view)
- [Rust CLI Book — Communicating with machines](https://rust-cli.github.io/book/in-depth/machine-communication.html)
- [serde-rs/serde — Security discussion: is Serde safe with untrusted input?](https://github.com/serde-rs/serde/issues/1087)
- [Rust Forum — serde_yaml deprecation and alternatives](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868)
- [Anchore — Beyond Cargo Audit: Securing Rust Crates in Container Images](https://anchore.com/blog/beyond-cargo-audit-securing-your-rust-crates-in-container-images/)
- [Sherlock — Rust Security & Auditing Guide 2026](https://sherlock.xyz/post/rust-security-auditing-guide-2026)
