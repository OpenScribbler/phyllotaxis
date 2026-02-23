# Security Review Report — phyllotaxis

**Prepared:** 2026-02-21
**Reviewer:** Maive (AI security reviewer)
**Codebase version:** Initial commit (all source files untracked, pre-v1)
**Scope:** Full source review against the Rust CLI Security Best Practices research report

---

## Summary

phyllotaxis is a read-only CLI tool that displays OpenAPI spec content to the terminal. Its security posture is reasonable for a developer-facing tool at this stage: it has no network access, no secrets storage, no privilege escalation paths, and makes correct use of `serde_json` for typed JSON output. The most significant risks are (1) ANSI escape code injection via untrusted strings from the OpenAPI spec being printed directly to the terminal without sanitization, and (2) a YAML injection vector in the `init` command that writes user-provided file paths into a config file using raw string interpolation. Secondary concerns include the use of a deprecated, unmaintained YAML parsing crate, non-atomic config file writes, and missing release profile hardening settings. There is no CI/CD security automation of any kind.

---

## Findings

---

### Finding 1: ANSI Escape Code Injection via Spec-Sourced Strings

**Severity:** High
**Location:** `src/render/text.rs` — lines 7, 10, 14–19, 32, 37, 71–73, 77–81, 114, 150, 166, 191–200, 237–248, 284–285, 285, 298–299, 304–305, 310–311, 320, 326–330, 403, 407–414, 416–427, 439, 443, 450, 459–461, 469, 476, 479, 504, 506, 532–533, 590, 554–560

**Issue:**

Every text-mode output function in `render/text.rs` interpolates strings sourced from the OpenAPI spec directly into `format!()` calls and prints the result to stdout. No sanitization of ANSI escape sequences is performed at any point.

The affected string sources (all originating from the untrusted spec file) include:

- API title (`data.title`) and description — `text.rs:7, 10`
- Server URLs — `text.rs:14–19`
- Auth scheme names and descriptions — `text.rs:499–506`
- Endpoint paths, summaries, and descriptions — `text.rs:69–73, 150, 459–461`
- Resource group slugs and descriptions — `text.rs:450, 590`
- Schema names and field descriptions — `text.rs:407–427`
- Parameter names and descriptions — `text.rs:192–200`
- Enum values — `text.rs:189, 234, 400`
- Discriminator property names and mapping values — `text.rs:320, 326–330`

A concrete example: `text.rs:504–506`:

```rust
out.push_str(&format!("    Scheme: {}\n", scheme.detail));
if let Some(ref desc) = scheme.description {
    out.push_str(&format!("    Description: {}\n", desc));
}
```

`scheme.detail` and `scheme.description` are decoded directly from the OpenAPI spec. A crafted spec can place `\x1b[2J` (clear screen), `\x1b]0;injected title\x07` (change window title), or more dangerous sequences in any of these fields.

**Why it matters:**

ANSI escape injection is a documented attack class with real CVEs. CVE-2021-25743 demonstrated ANSI injection in Kubernetes. The `tracing-subscriber` Rust crate had a confirmed ANSI injection vulnerability in 2025. A malicious OpenAPI spec — whether crafted intentionally or obtained from a compromised source — can manipulate the developer's terminal when they run `phyllotaxis`. Consequences range from cosmetic (overwriting previous output, spoofing terminal content) to dangerous in older terminals (input injection). Developers using phyllotaxis to inspect third-party or vendor-supplied specs are particularly exposed.

**Recommendations:**

1. **Strip ANSI escape sequences from all spec-sourced strings before text rendering.** Add a sanitization function called before any spec string is interpolated into text output. The minimal approach strips `\x1b` (ESC, 0x1B) and all sequences starting with it:

   ```rust
   fn sanitize(s: &str) -> std::borrow::Cow<str> {
       if s.contains('\x1b') {
           std::borrow::Cow::Owned(s.replace('\x1b', ""))
       } else {
           std::borrow::Cow::Borrowed(s)
       }
   }
   ```

   A more complete approach also strips `\x9b` (CSI in single-byte encoding) and `\x07` (BEL, used in OSC sequences). The `strip-ansi-escapes` crate provides a well-tested byte-level filter as an alternative.

2. **Conditionally sanitize only when stdout is a terminal.** When piped, the consumer handles interpretation; the risk is only when a human is reading the terminal. Use `std::io::IsTerminal` to detect this:

   ```rust
   use std::io::IsTerminal;
   let value = if std::io::stdout().is_terminal() {
       sanitize(raw_value)
   } else {
       std::borrow::Cow::Borrowed(raw_value)
   };
   ```

   Trade-off: option 1 is simpler and always safe; option 2 preserves ANSI if the caller explicitly pipes to a tool that understands it, but adds branching complexity and a `std::io::IsTerminal` import.

3. **Apply sanitization at the model boundary, not at each render site.** Add sanitization when spec data is moved into internal model structs (in `commands/resources.rs`, `commands/auth.rs`, etc.) rather than in each `format!` call. This is safer because it ensures all render paths (including future ones) benefit automatically.

---

### Finding 2: YAML Injection in Config File Write (init command)

**Severity:** High
**Location:** `src/commands/init.rs:168`, `src/commands/init.rs:231`

**Issue:**

The `init` command writes a `.phyllotaxis.yaml` config file by interpolating a user-supplied file path string directly into a YAML string using `format!`:

```rust
// init.rs:168
let content = format!("spec: {}\n", relative);
std::fs::write(&config_path, content).expect("failed to write .phyllotaxis.yaml");
```

And in `run_add_spec` at line 231:

```rust
let new_line = format!("  {}: {}", name, relative);
```

Here both `name` (the spec name the user typed) and `relative` (derived from user path input) are inserted raw into YAML content. Neither is quoted or escaped.

A user (or an automated script driving `phyllotaxis init`) who enters a path containing YAML special characters can corrupt the config file. More significantly, a path containing a newline (`\n`) followed by arbitrary YAML would inject additional keys or nested structure into the config. For example, entering a "path" of:

```
./openapi.yaml
variables:
  tenant: injected
```

would produce a config file with an injected `variables` block that phyllotaxis will then parse and act on during subsequent `overview` runs.

`name` in `run_add_spec` accepts arbitrary stdin input with no validation at line 183 (`let name = name_input.trim()`). A name containing `:` would break the YAML mapping syntax. A name containing newlines would inject additional YAML keys.

**Why it matters:**

The `variables` map from the config is used in `commands/overview.rs:45–48` to substitute into server URL templates. A successfully injected variable could manipulate displayed server URLs. More broadly, YAML injection into a config file that the same tool then reads is a confused-deputy-style issue: the tool writes attacker-controlled content, then reads and acts on it as trusted config.

**Recommendations:**

1. **Quote all user-provided strings before inserting into YAML.** The simplest safe fix is to wrap the value in double quotes and escape any embedded double quotes or backslashes:

   ```rust
   fn yaml_quote(s: &str) -> String {
       format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
   }
   let content = format!("spec: {}\n", yaml_quote(&relative));
   ```

2. **Build the YAML structure using `serde_yaml` serialization, not string formatting.** Serialize a `Config` struct and write the result. This guarantees correct escaping for all values regardless of content:

   ```rust
   let cfg = Config { spec: Some(relative), ..Default::default() };
   let content = serde_yaml::to_string(&cfg)?;
   fs::write(&config_path, content)?;
   ```

   Trade-off: this requires `Config` to derive `Serialize` in addition to `Deserialize`. It is the more robust approach and eliminates the entire class of injection.

3. **Reject input containing newlines or other YAML-dangerous characters.** As a lightweight defense-in-depth measure, validate `name` and `path` inputs before use. Reject strings containing `\n`, `\r`, `:` at the start of `name`, or other YAML structure characters.

---

### Finding 3: Deprecated and Unmaintained serde_yaml Dependency

**Severity:** High
**Location:** `Cargo.toml:10`

**Issue:**

```toml
serde_yaml = "0.9"
```

`serde_yaml 0.9` is the final version of the original `serde_yaml` crate. Its author (dtolnay) deprecated the crate in mid-2024 and archived it. The crate receives no security fixes, no maintenance, and no updates. It is in active use at three locations in the codebase:

- `spec.rs:31` — `serde_yaml::from_str::<Config>(&content)` — parses the config file
- `spec.rs:177` — `serde_yaml::from_str(&content)` — parses the OpenAPI spec
- `commands/auth.rs:137`, `commands/resources.rs:561`, `commands/schemas.rs:215` — test helpers, but the same code path is used in production via the fixture loader

The `serde_yml` fork (a popular migration target for `serde_yaml`) received **RUSTSEC-2025-0068** for a segmentation fault in its serializer. While this specific advisory is against `serde_yml` and not `serde_yaml 0.9`, it illustrates that this area of the ecosystem is actively developing new security issues. An unmaintained crate cannot receive patches for such discoveries.

**Why it matters:**

Using an unmaintained crate means that any future vulnerability discovered in `serde_yaml` will go unpatched. The crate parses untrusted user-supplied YAML (the OpenAPI spec) and config-file YAML. YAML parsing has historically been a source of vulnerabilities (deserialization attacks, stack overflows from deeply nested structures, alias bombs). An unmaintained crate will not receive fixes for issues of this type as they are discovered.

**Recommendations:**

1. **Migrate to `serde_yaml_ng`.** This is the most active maintained fork of `serde_yaml` with a nearly identical API. The migration is typically a find-and-replace of the crate name in `Cargo.toml` and `use` statements. API compatibility is high for the subset of features phyllotaxis uses (`from_str`).

2. **Migrate to `serde_norway`.** An alternative maintained fork. Less widely adopted than `serde_yaml_ng` but actively maintained.

3. **Run `cargo audit` now and after migration.** The RustSec advisory database should be checked immediately to confirm no advisory exists against the exact `serde_yaml` version in `Cargo.lock`, and again after migrating to confirm the replacement is clean.

---

### Finding 4: Non-Atomic Config File Writes

**Severity:** Medium
**Location:** `src/commands/init.rs:169`, `src/commands/init.rs:248`

**Issue:**

The `init` command writes the config file using `std::fs::write` directly to the destination path:

```rust
// init.rs:169
std::fs::write(&config_path, content).expect("failed to write .phyllotaxis.yaml");
```

```rust
// init.rs:248
std::fs::write(config_path, lines.join("\n") + "\n").expect("failed to write config");
```

`fs::write` is a non-atomic operation: it truncates the file, then writes the new content. If the process is killed or the system crashes between the truncation and the completion of the write, the config file is left in a corrupt or empty state. The existing config is destroyed at truncation before the new content is confirmed written.

Additionally, `run_add_spec` at lines 227–246 reads the existing config, modifies it in memory, then overwrites the file. There is a window between the read (line 227) and the write (line 248) during which another process could modify the file — a TOCTOU race on the config.

**Why it matters:**

For a config file that stores the path to the user's OpenAPI spec(s), corruption means the tool becomes non-functional until the user manually recreates the config. This is primarily a reliability issue, but it has a security dimension: if `init` is run as part of an automated setup script, a partial write could leave the config in a state that causes `phyllotaxis` to read an unintended spec file during subsequent runs.

**Recommendations:**

1. **Use an atomic write pattern (write-to-temp, then rename).** `fs::rename` is atomic on POSIX systems when source and destination are on the same filesystem (which `.yaml.tmp` and `.yaml` in the same directory always are):

   ```rust
   let tmp_path = config_path.with_extension("yaml.tmp");
   std::fs::write(&tmp_path, content)?;
   std::fs::rename(&tmp_path, &config_path)?;
   ```

   Trade-off: adds one extra file operation and a `tmp` file briefly on disk. The `tmp` file could be left behind on crash before the rename, requiring cleanup logic — or it can simply be overwritten on the next `init` run.

2. **Accept the current behavior as a known limitation.** For a CLI tool where `init` is run interactively by the developer who controls their own machine, a non-atomic write is unlikely to cause security issues and only rarely causes reliability problems. If the complexity of the atomic write pattern is not warranted at this stage, document the limitation.

---

### Finding 5: TOCTOU Race Conditions in Path Checks

**Severity:** Medium
**Location:** `src/spec.rs:23`, `src/spec.rs:92`, `src/spec.rs:133`; `src/commands/init.rs:117`

**Issue:**

Several locations check a filesystem property and then act on that check, with a window between them:

**spec.rs:23** — config discovery:
```rust
if config_path.is_file() {
    let content = match std::fs::read_to_string(&config_path) { ... }
```

**spec.rs:92** — spec path resolution:
```rust
if resolved.is_file() {
    return Ok(resolved);
}
```

**spec.rs:133** — backward-compat spec resolution:
```rust
if resolved.is_file() {
    return Ok(resolved);
}
```

**init.rs:117** — init branching:
```rust
if config_path.exists() {
    run_add_spec(start_dir, &config_path);
    return;
}
```

In each case, the check and the subsequent action (read or write) are not atomic. Between the check and the action, the filesystem state can change. A symlink could be substituted for the file, or the file could be replaced.

**Why it matters:**

For a CLI tool where the user running the tool is also the filesystem owner, the practical exploitability is very low — an attacker would need to have write access to the directory, in which case they could simply modify the spec directly. However, `spec.rs:23` and `init.rs:117` operate on the current working directory, which is user-controlled. The pattern is worth eliminating as best practice, and it becomes relevant if phyllotaxis is ever run in a context with shared directory permissions (CI runners, shared developer environments).

The documented Rust standard library TOCTOU CVE-2022-21658 in `remove_dir_all` demonstrates that this pattern causes real vulnerabilities even in core Rust code.

**Recommendations:**

1. **Replace check-then-act with act-and-handle-error.** Rather than `if path.is_file() { read(path) }`, call `read(path)` directly and handle `ErrorKind::NotFound`:

   ```rust
   match fs::read_to_string(&config_path) {
       Ok(content) => { /* parse */ }
       Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
       Err(e) => { eprintln!("Warning: ..."); return None; }
   }
   ```

2. **Accept the current pattern for the spec path resolution cases.** The spec path resolution checks (`spec.rs:92`, `spec.rs:133`) are lower risk because they only determine whether a path is valid before returning it to the caller — the actual file read happens separately and is the authoritative check. Eliminating the TOCTOU entirely for these cases would require restructuring the resolution logic significantly for marginal gain.

---

### Finding 6: Error Messages Leak Internal Filesystem Paths

**Severity:** Medium
**Location:** `src/spec.rs:77–80`, `src/spec.rs:136–140`, `src/spec.rs:173–174`, `src/spec.rs:179`; `src/commands/init.rs:176`

**Issue:**

Several error messages and warnings include internal filesystem paths that were derived from config file processing:

**spec.rs:77–80** — named spec not found:
```rust
return Err(format!(
    "Named spec '{}' points to '{}' which was not found (resolved from {})",
    spec,
    named_path,
    config_dir.display()   // full absolute path leaked
));
```

**spec.rs:136–140**:
```rust
return Err(format!(
    "Spec file from config not found: {} (resolved from {})",
    resolved.display(),    // full absolute path
    config_dir.display()   // full absolute path
));
```

**spec.rs:173–174**:
```rust
.map_err(|e| format!("Failed to read {}: {}", spec_path.display(), e))?;
```

**spec.rs:179**:
```rust
.map_err(|e| format!("Failed to parse {}: {}", spec_path.display(), e))?;
```

The `serde_yaml::Error` passed as `e` in line 179 may include internal parser state details.

**init.rs:176**:
```rust
eprintln!("Config already exists at {}.", config_path.display());
```

**Why it matters:**

For a single-user CLI tool running as the user themselves, exposing their own filesystem paths back to them is harmless and actually helpful. However, these error messages could become a concern if phyllotaxis output is logged, captured in CI artifacts, or displayed in shared environments. The `serde_yaml::Error` in `spec.rs:179` is the highest-risk case — parser error messages from YAML libraries have in some historical cases included portions of the parsed content, which could expose sensitive data from a config file in an error log.

The path exposure in `spec.rs:77–80` (`config_dir.display()`) is notable because it exposes not just the path the user provided but the internally resolved path including the config discovery walk — information the user did not explicitly supply.

**Recommendations:**

1. **Sanitize `serde_yaml::Error` in parse failure messages.** Rather than forwarding the raw error object, emit a fixed message:

   ```rust
   .map_err(|_| format!("Failed to parse {}: invalid YAML or OpenAPI format", spec_path.display()))?;
   ```

2. **Remove internal resolution details from user-facing messages.** The `"resolved from {}"` clause in `spec.rs:77–80` and `spec.rs:136–140` is useful for debugging but leaks internal state. Consider moving it behind a `--verbose` flag or removing it.

3. **Accept current path exposure for the single-user CLI use case.** The paths phyllotaxis exposes are paths the user already knows (they configured them). The risk is low for the intended use case. Prioritize the serde_yaml error case as the one worth addressing.

---

### Finding 7: `expect` Calls on User-Influenced Data Paths

**Severity:** Medium
**Location:** `src/main.rs:62`, `src/main.rs:70`; `src/commands/init.rs:147–148`, `src/commands/init.rs:181–182`, `src/commands/init.rs:206–207`; `src/render/json.rs:63`, `src/render/json.rs:100`, `src/render/json.rs:157`, `src/render/json.rs:174`, `src/render/json.rs:308`, `src/render/json.rs:312`, `src/render/json.rs:316`, `src/render/json.rs:321`

**Issue:**

Multiple `expect` calls occur on operations that could plausibly fail in user-facing scenarios:

**main.rs:62:**
```rust
let cwd = std::env::current_dir().expect("cannot determine current directory");
```

**main.rs:70:**
```rust
let spec_flag = cli.spec.as_ref().map(|p| p.to_str().expect("spec path not valid UTF-8"));
```

If the user supplies a `--spec` path containing non-UTF-8 bytes (which `OsStr` allows on Linux), this call panics with a Rust backtrace containing internal file paths, function names, and line numbers — all useful to an attacker performing reconnaissance.

**init.rs:147–148:**
```rust
std::io::stdin()
    .read_line(&mut input)
    .expect("failed to read input");
```

If stdin is closed or returns an error, this panics rather than exiting cleanly. This affects `init.rs:181–182` and `206–207` as well.

**render/json.rs — multiple locations:**
```rust
serde_json::to_string_pretty(&json).expect("serialize overview")
```

These `expect` calls on `serde_json::to_string_pretty` are low-risk because serialization of these well-typed structs will not fail in practice (no `f32::NAN` or similar values that serde_json rejects). However, the pattern of calling `expect` on serialization is worth noting as a habit that could break if the data types change.

**Why it matters:**

A panic produces a Rust backtrace by default when `RUST_BACKTRACE=1` is set, and may produce one even without it depending on the panic handler. Backtraces include: file paths (of Rust source files), function names, crate names, and line numbers. This is less critical for a single-user CLI than for a server, but the non-UTF-8 path case at `main.rs:70` is a real failure mode on Linux that produces a poor user experience at minimum and information leakage at most.

**Recommendations:**

1. **Replace `main.rs:70` `expect` with graceful error handling.** Non-UTF-8 paths are valid on Linux. Use `p.to_str()` with a proper error return:

   ```rust
   let spec_flag = cli.spec.as_ref()
       .map(|p| p.to_str().ok_or("spec path contains non-UTF-8 characters"))
       .transpose()
       .unwrap_or_else(|e| die(e));
   ```

2. **Replace `init.rs` stdin `expect` calls with error handling that exits cleanly.** A closed stdin should produce a clean error message, not a panic.

3. **The `render/json.rs` `expect` calls are low priority.** They are logically infallible given the current types. Consider converting them to `unwrap_or_else(|_| String::from("{}"))` or using `?` propagation if the render functions are ever refactored to return `Result`.

---

### Finding 8: No File Size Limit on OpenAPI Spec Parsing

**Severity:** Medium
**Location:** `src/spec.rs:173`

**Issue:**

```rust
let content = std::fs::read_to_string(&spec_path)
    .map_err(|e| format!("Failed to read {}: {}", spec_path.display(), e))?;
```

The entire spec file is read into memory without any size check. The subsequent `serde_yaml::from_str` call at line 177 parses the entire content recursively. A maliciously crafted YAML file with deeply nested structures (such as thousands of levels of nested mappings) can cause a stack overflow during recursive descent parsing, crashing the process. YAML also supports "alias" references (`&anchor`/`*alias`) that, when nested, can cause exponential memory expansion — the "YAML bomb" pattern analogous to a zip bomb.

**Why it matters:**

For a single-user CLI where the user controls what spec file is loaded, the practical risk is low — a user who supplies a malicious spec harms only themselves. The risk increases in two scenarios: (1) phyllotaxis is integrated into a script or CI pipeline that fetches specs from external sources, and (2) a developer is asked to "just run this tool against our API spec" and the spec is malicious. Denial-of-service via stack overflow is the likely outcome. The `openapiv3` crate's recursive schema resolution (via `expand_fields` in `commands/schemas.rs`) adds an additional recursion path, though that one is already bounded by the `max_depth: 5` parameter.

**Recommendations:**

1. **Add a file size check before reading.** Reject files above a reasonable limit (e.g., 50 MB):

   ```rust
   let metadata = std::fs::metadata(&spec_path)
       .map_err(|e| format!("Failed to stat {}: {}", spec_path.display(), e))?;
   if metadata.len() > 50 * 1024 * 1024 {
       return Err(format!("Spec file is too large ({} bytes). Maximum is 50 MB.", metadata.len()));
   }
   ```

2. **Accept the current behavior as a known limitation for a developer tool.** If phyllotaxis's intended use is loading known-good specs in developer workflows, the risk profile is acceptable and the added code complexity may not be warranted at this stage.

---

### Finding 9: Missing Release Profile Security Settings

**Severity:** Medium
**Location:** `Cargo.toml` — no `[profile.release]` section present

**Issue:**

`Cargo.toml` contains no `[profile.release]` section. This means the release build uses all Cargo defaults:

- `overflow-checks = false` — integer arithmetic in release silently wraps on overflow rather than panicking. Wrapping overflow can cause logic errors in bounds calculations, length math, or index arithmetic. phyllotaxis computes `max_name` and `max_type` column widths by iterating over collections from the spec (e.g., `text.rs:183`, `text.rs:208`, `text.rs:374–375`). If spec data were crafted to trigger integer overflow in these calculations, the display formatting could produce unexpected output.
- `strip = false` — release binaries include debug information (symbol names, file paths, line numbers) by default. This helps reverse engineers understand the binary structure and aids attackers in identifying specific code locations to target.
- `lto = false` — link-time optimization is disabled. LTO reduces binary size and eliminates dead code, which reduces the available attack surface in the compiled binary.

**Why it matters:**

`overflow-checks` is the most security-relevant default to override. The others (strip, lto) are hardening measures that reduce information exposure and attack surface. For a tool at this stage, missing these settings is a missed opportunity rather than an active vulnerability.

**Recommendations:**

Add a `[profile.release]` section to `Cargo.toml`:

```toml
[profile.release]
overflow-checks = true   # Panic on integer overflow instead of wrapping silently
strip = "debuginfo"      # Strip debug info; reduces information in distributed binary
lto = true               # Dead code elimination, reduced binary size
```

The `overflow-checks = true` setting is the highest priority. It has negligible performance cost for a CLI tool and prevents an entire class of logic errors. `strip = "debuginfo"` and `lto = true` are best-practice hardening for distributed binaries.

---

### Finding 10: No CI/CD Security Automation

**Severity:** Medium
**Location:** Repository root — no `.github/workflows/` directory

**Issue:**

There is no CI/CD configuration of any kind. No automated checks run on commit or pull request. Specifically absent:

- `cargo audit` — no automated check against the RustSec advisory database. New advisories are published continuously; the project will not be notified if a dependency receives a new CVE.
- `cargo deny` — no license compliance, crate ban, or source validation checks.
- `cargo clippy -- -D warnings` — no lint enforcement in CI. Clippy catches patterns that lead to security issues (incorrect bounds checks, potential panics, redundant clones).
- `cargo test` — no automated test runner to gate on test failures.
- Scheduled dependency audits — even if a passing `cargo audit` is added to push-triggered CI, new advisories published after the last push will go undetected without a scheduled run.

**Why it matters:**

The `serde_yaml` deprecation (Finding 3) is the concrete current risk: `cargo audit` would flag the unmaintained status. A daily scheduled `cargo audit` run would surface any new advisory against any current dependency before it becomes a production concern. For a project at this stage, adding CI is the single highest-leverage security improvement available.

**Recommendations:**

1. **Add a GitHub Actions workflow with `cargo audit` on push and on a daily schedule.** The `rustsec/audit-check` action is the minimal implementation:

   ```yaml
   # .github/workflows/security.yml
   on:
     push:
       paths: ['Cargo.toml', 'Cargo.lock']
     schedule:
       - cron: '0 9 * * 1'  # Monday 9am UTC
   jobs:
     audit:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - uses: rustsec/audit-check@v2
           with:
             token: ${{ secrets.GITHUB_TOKEN }}
   ```

2. **Add `cargo clippy -- -D warnings` to a standard CI workflow.** This should run on every push and pull request alongside `cargo test`.

3. **Add `cargo deny` for comprehensive dependency governance.** Requires a `deny.toml` configuration file but provides license, ban, and source checking in addition to advisory detection.

---

### Finding 11: Spec Path Not Canonicalized Before Use

**Severity:** Low
**Location:** `src/spec.rs:86–92`, `src/spec.rs:127–134`

**Issue:**

When `--spec` is provided as a relative path, it is resolved relative to `start_dir` using `start_dir.join(path)` without subsequent canonicalization:

```rust
// spec.rs:86-92
let path = PathBuf::from(spec);
let resolved = if path.is_absolute() {
    path
} else {
    start_dir.join(path)
};
if resolved.is_file() {
    return Ok(resolved);
}
```

`PathBuf::join` does not resolve `..` components. A path like `../../etc/passwd` joined to `start_dir` produces `start_dir/../../etc/passwd` — which `Path::join` returns as-is without resolving the traversal. The actual filesystem resolution happens when the OS opens the file, so path traversal sequences do work in practice.

Similarly, spec paths in the config file (`cfg.specs` map values, `cfg.spec` single field) are joined to `config_dir` without canonicalization.

**Why it matters:**

For a CLI tool where the user controls both the `--spec` flag and their own filesystem, path traversal is the user harming themselves — low severity. The risk model shifts if phyllotaxis is run in a context where the spec path comes from a partially trusted source (a config file checked in by a teammate, an automated pipeline that passes the spec flag from external input). Even then, the user running the tool has the same filesystem permissions as the tool, so traversal accesses files they could read directly.

The more realistic concern is that the error messages at `spec.rs:77–80` and `spec.rs:136–140` expose the unresolved path including `..` sequences, which may look confusing to users.

**Recommendations:**

1. **Canonicalize spec paths before returning them from `resolve_spec_path`.** Call `fs::canonicalize` on the resolved path and verify it resolves to a real file:

   ```rust
   let canonical = fs::canonicalize(&resolved)
       .map_err(|_| format!("Spec '{}' not found.", spec))?;
   return Ok(canonical);
   ```

   Trade-off: `canonicalize` requires the file to exist (it calls `stat`), which is already implied by the `is_file()` check. This change eliminates the TOCTOU on the path check simultaneously (the path returned is the canonical filesystem path at time of resolution).

2. **Accept the current behavior.** For the intended developer CLI use case, this is very low risk. Canonicalization adds marginal security benefit at the cost of adding another syscall per resolution.

---

### Finding 12: Cargo.lock Not Explicitly Committed (Informational)

**Severity:** Informational
**Location:** Repository root — `Cargo.lock` is present in the untracked files list but `.gitignore` is also untracked

**Issue:**

`Cargo.lock` is present in the repository directory (visible in the git status as untracked). However, because `.gitignore` is also currently untracked, it is unclear whether `Cargo.lock` will be committed to version control or excluded.

For binary crates (which phyllotaxis is), `Cargo.lock` should be committed. It pins exact dependency versions and ensures reproducible builds. Without it, `cargo install phyllotaxis` or a fresh `cargo build` on a different machine may pull different patch versions of dependencies than the developer tested — including versions that may introduce vulnerabilities.

**Why it matters:**

If `Cargo.lock` is excluded from version control, two risks arise: (1) users who install phyllotaxis from source may get different (potentially vulnerable) dependency versions, and (2) `cargo audit` run in CI operates on `Cargo.lock` — without it committed, CI cannot pin its audit to a known dependency state.

**Recommendations:**

1. **Ensure `Cargo.lock` is committed for this binary crate.** Verify that `.gitignore` does not exclude it. The Cargo documentation explicitly recommends committing `Cargo.lock` for binary executables.

---

## Prioritized Action List

The following is ordered by risk to users of the tool, highest priority first.

1. **[HIGH] Sanitize ANSI escape sequences in all text-mode output (Finding 1).** This is the highest-impact, most exploitable issue. A malicious or compromised OpenAPI spec can manipulate the developer's terminal. Apply a sanitization pass to all spec-sourced strings before they are rendered in text mode.

2. **[HIGH] Fix YAML injection in the init command config write (Finding 2).** User-controlled input is written raw into a YAML config file. Use `serde_yaml` serialization (or at minimum YAML quoting) when constructing config file content, and validate the spec name input for illegal YAML characters.

3. **[HIGH] Migrate away from deprecated serde_yaml 0.9 (Finding 3).** The crate is archived and unmaintained. Migrate to `serde_yaml_ng` or `serde_norway`. Run `cargo audit` immediately before and after migration.

4. **[MEDIUM] Add CI/CD with cargo audit on push and scheduled weekly (Finding 10).** This is the highest-leverage infrastructure change. A daily or weekly scheduled audit catches new advisories against existing dependencies without requiring any code change. Add `cargo clippy -- -D warnings` and `cargo test` to push-triggered CI.

5. **[MEDIUM] Add `[profile.release]` with `overflow-checks = true` (Finding 9).** Zero performance cost for a CLI, eliminates a class of silent arithmetic bugs.

6. **[MEDIUM] Fix `expect` on non-UTF-8 path input (Finding 7, specifically main.rs:70).** Replace with graceful error handling that exits cleanly instead of panicking with a backtrace.

7. **[MEDIUM] Use atomic writes in the init command (Finding 4).** Write-to-temp then rename to prevent partial config writes on crash.

8. **[MEDIUM] Add file size check before parsing OpenAPI specs (Finding 8).** Prevents denial-of-service from oversized or maliciously crafted spec files.

9. **[LOW] Sanitize serde_yaml parse error details in user-facing messages (Finding 6, spec.rs:179).** Replace the forwarded serde_yaml::Error with a fixed message to prevent parser internals from appearing in output.

10. **[LOW] Replace check-then-act filesystem patterns with act-and-handle-error (Finding 5).** Lower priority given the single-user CLI context, but worth addressing for correctness.

11. **[LOW] Canonicalize spec paths before returning from resolve_spec_path (Finding 11).** Low practical risk for the intended use case, but eliminates path traversal sequences from error messages and return values.

12. **[INFORMATIONAL] Confirm Cargo.lock is committed to version control (Finding 12).** Verify `.gitignore` does not exclude it.
