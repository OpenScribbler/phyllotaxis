# UX Review Report — phyllotaxis

**Date:** 2026-02-21
**Reviewer:** UX Review (against `docs/reviews/rust-ux-research-report.md`)
**Scope:** Full codebase — all source files in `src/`

---

## Summary

phyllotaxis has a well-structured command hierarchy and correctly funnels all diagnostic output through `eprintln!` rather than `println!`, which is the single most important stdout hygiene rule for a pipelined CLI. The progressive disclosure model (overview → resource list → resource detail → endpoint detail) is coherent and well-executed. However, the tool is missing several table-stakes UX features that affect scriptability and first-run experience: JSON output always uses pretty-printing regardless of TTY state, errors in `--json` mode are plain text on stderr instead of JSON, there is no TTY detection or color at all, the "did you mean" phrasing is inconsistent with current Rust ecosystem conventions, the `init` command has no non-interactive flag path and will hang in CI, and the JSON output for list views wraps data in an envelope object that breaks idiomatic `jq` usage. None of these are correctness bugs in the spec-parsing logic, but they collectively limit the tool's usefulness in automated workflows and reduce the quality of the user experience for interactive use.

---

## Findings

### Finding 1: JSON list commands wrap output in an envelope object instead of returning a bare array

**Severity:** High
**Location:** `src/render/json.rs:66–101`, `src/render/json.rs:160–175`
**Issue:** `render_resource_list` returns `{"resources": [...], "drill_deeper": "..."}` and `render_schema_list` returns `{"schemas": [...], "total": N, "drill_deeper": "..."}` rather than bare JSON arrays. A user running `phyllotaxis resources --json | jq '.[0]'` gets `null` because the top-level value is an object, not an array. The `drill_deeper` and `total` keys also embed prose guidance and metadata into machine-readable output, which is only appropriate for human-readable output.

**Why it matters:** The research report is explicit: "Commands that return lists should return a JSON array at the top level when `--json` is used. Do not wrap in an envelope object unless metadata (like pagination) must accompany the result." phyllotaxis's stated goal is to be a building block for larger workflows. Wrapping list output breaks the most common `jq` usage patterns and forces consumers to know the internal envelope key name.

**Recommendations:**
1. Return a bare JSON array for all list commands. `render_resource_list` returns `serde_json::to_string_pretty(&resources_vec)`, `render_schema_list` returns `serde_json::to_string_pretty(&names)`. Remove `drill_deeper` and `total` from JSON output entirely — they are human guidance, not data.
2. If metadata (total count, pagination cursor) genuinely needs to accompany list results in future, use an envelope only then and document it as a deliberate schema choice. For now there is no pagination so there is no justification for the envelope.

---

### Finding 2: JSON output is always pretty-printed regardless of TTY state

**Severity:** High
**Location:** `src/render/json.rs:63`, `src/render/json.rs:100`, `src/render/json.rs:157`, `src/render/json.rs:174`, `src/render/json.rs:308`, `src/render/json.rs:312`, `src/render/json.rs:316`, `src/render/json.rs:321`
**Issue:** Every JSON render function unconditionally calls `serde_json::to_string_pretty(...)`. When stdout is a pipe (the primary use case for `--json` mode), pretty-printing adds whitespace overhead that inflates output size for large specs and wastes cycles. The research report states: "Default to compact (no extra whitespace) when piped; use pretty-printed when stdout is a TTY and `--json` is explicitly requested."

**Why it matters:** A user running `phyllotaxis resources --json | jq '.[0]'` does not benefit from pretty-printing — `jq` re-formats it anyway. More importantly, pretty-printing as the unconditional default signals that the tool does not distinguish between interactive and automated contexts, which matters to users who are evaluating it for scripting.

**Recommendations:**
1. Add the `is-terminal` crate and check `std::io::stdout().is_terminal()` at the call site in `main.rs`. Pass a `pretty: bool` flag into each render function, or have a single `json_render` helper that selects `to_string_pretty` vs `to_string`.
2. Use `to_string_pretty` when stdout is a TTY and `--json` was requested (the user is inspecting output interactively), and `to_string` (compact) when stdout is a pipe. This is a one-line change per render function once the TTY check is in place.

---

### Finding 3: Errors in `--json` mode are plain text on stderr, not JSON

**Severity:** High
**Location:** `src/main.rs:56–58` (`die` function), `src/main.rs:128–135` (resource not found), `src/main.rs:171–179` (schema not found), `src/spec.rs:26–29`, `src/spec.rs:33–36`
**Issue:** When `--json` is active and an error occurs — for example, a schema is not found — the error is emitted as a plain-text `eprintln!` string: `"Error: Schema 'Foo' not found."`. A script consuming JSON output from phyllotaxis must handle two entirely different error formats on stderr depending on which error occurred. The `die` function at `main.rs:56` also always emits plain text regardless of `--json`. Crucially, `die` is called before the `--json` flag is even checked for the spec-load error at `main.rs:73`.

**Why it matters:** The research report is direct: "When `--json` is active, errors should also be JSON rather than plain text on stderr." Tools like the AWS CLI and GitHub CLI implement this pattern because it makes the tool fully scriptable — a consuming script can `2>&1 | jq .error` to detect failure conditions without text parsing.

**Recommendations:**
1. Pass `cli.json` into `die` (or replace it with a context-aware error handler) so it emits `{"error": "...", "message": "..."}` to stderr when in JSON mode.
2. At the resource-not-found and schema-not-found blocks in `main.rs` (lines 128–135 and 171–179), check `cli.json` and emit either structured JSON or plain text accordingly.
3. A minimal structured error format: `{"error": "not_found", "message": "Schema 'Foo' not found", "suggestion": "FooBar"}`. The suggestion field is only present when similar matches exist.

---

### Finding 4: "Did you mean" phrasing is used; Rust ecosystem convention is declarative

**Severity:** Medium
**Location:** `src/main.rs:130`, `src/main.rs:172`
**Issue:** Both the resource-not-found and schema-not-found error paths emit `"Did you mean:"` followed by corrected commands. The exact lines are:
```
eprintln!("Did you mean:");
for s in &suggestions {
    eprintln!("  phyllotaxis resources {}", s);
}
```
The research report, citing the Rust compiler diagnostics guide, recommends against this phrasing because "it frames a statement of fact as a question, which can feel uncertain." The preferred alternative is a declarative statement.

**Why it matters:** This is a polish issue, not a functional one. The phrasing "Did you mean" reads as uncertain, as if the tool is unsure of its own suggestion. A declarative format ("a resource with a similar name exists") is more confident and aligns with how `rustc`, `cargo`, and the broader Rust ecosystem communicate suggestions.

**Recommendations:**
1. Change the phrasing at `main.rs:130` to:
   ```
   eprintln!("note: a resource with a similar name exists:");
   for s in &suggestions {
       eprintln!("    phyllotaxis resources {}", s);
   }
   ```
2. Apply the same change at `main.rs:172` for schemas.
3. If `miette` is adopted for error formatting (see Finding 5), this can be expressed as a `help:` label in the diagnostic output naturally.

---

### Finding 5: Suggestion matching uses substring containment, not edit distance

**Severity:** Medium
**Location:** `src/commands/resources.rs:543–551`, `src/commands/schemas.rs:55–68`
**Issue:** Both `suggest_similar` (resources) and `suggest_similar_schemas` (schemas) use `.contains()` substring matching:
```rust
// resources.rs:546
.filter(|g| g.slug.to_lowercase().contains(&slug_lower))

// schemas.rs:59
.filter(|k| k.to_lowercase().contains(&lower))
```
This means a typo like `phyllotaxis resources ptes` (transposing "pets") produces no suggestion because `"pets"` does not contain `"ptes"`. Substring matching only helps when the user typed a prefix or partial name, not when they made a spelling error.

**Why it matters:** The primary use case for suggestions is typo correction. Substring containment does not catch transpositions, deletions, or insertions — the most common typo classes. The research report explicitly recommends `strsim` with Levenshtein or OSA distance and a threshold of 3 edits.

**Recommendations:**
1. Add `strsim = "0.11"` to `Cargo.toml` and replace the substring filter with an edit-distance filter in both `suggest_similar` and `suggest_similar_schemas`. A threshold of `<= 3` (or `<= name.len() / 3` for length-normalized cutoff) is a reasonable starting point.
2. Keep substring matching as a secondary pass if edit-distance returns nothing, since both can run cheaply on the small candidate sets typical in an OpenAPI spec.

---

### Finding 6: No TTY detection — no color, no adaptive formatting

**Severity:** Medium
**Location:** `src/render/text.rs` (entire file), `Cargo.toml`
**Issue:** The text renderer uses no color whatsoever. `[DEPRECATED]` and `[ALPHA]` markers in `render_resource_list` (lines 582–588), `render_resource_detail` (lines 546–552), and `render_endpoint_detail` (lines 62–68) are plain uppercase ASCII text with no color distinction. There is also no TTY detection — no check of `NO_COLOR`, `TERM=dumb`, or whether stdout is a terminal. Neither `owo-colors`, `colored`, nor `is-terminal` appear in `Cargo.toml`.

**Why it matters:** Color serves a functional purpose here: deprecated resources should visually stand out (yellow or red), alpha resources are distinct from stable ones. Without color, the markers are easy to miss when scanning a long list. Without TTY detection, if color were added naively it would emit ANSI escape codes into log files and pipe output, which breaks downstream consumers. Both problems need to be addressed together.

**Recommendations:**
1. Add `owo-colors` (not `colored` — see the research report's "crates to avoid" section) and `is-terminal` to `Cargo.toml`.
2. Use `owo-colors`'s `if_supports_color` method, which automatically checks `NO_COLOR`, `TERM=dumb`, and TTY state. Apply it only in the text renderer, never in the JSON renderer.
3. Priority coloring targets: `[DEPRECATED]` in yellow/red, `[ALPHA]` in yellow, error labels in red, section headers ("Resources:", "Fields:", etc.) in bold, and resource/schema names in a consistent accent color.

---

### Finding 7: `init` command has no non-interactive flag path and will hang in CI

**Severity:** Medium
**Location:** `src/commands/init.rs:114–172`
**Issue:** `run_init` unconditionally reads from `stdin` at lines 145–149:
```rust
let mut input = String::new();
std::io::stdin()
    .read_line(&mut input)
    .expect("failed to read input");
```
There is no check of whether stdin is a TTY before prompting. If `phyllotaxis init` is run in a CI pipeline, a Makefile, or any non-interactive context where stdin is a pipe or `/dev/null`, the `read_line` call will either block indefinitely or return an empty string, producing an invalid config (`spec: \n`). There is also no `--spec-path` flag to provide the spec path non-interactively.

The `Init` subcommand definition in `main.rs:51` is simply `Init,` with no arguments. The research report is explicit: "Every interactive prompt must have a non-interactive flag equivalent."

**Why it matters:** Any user trying to use phyllotaxis in a team onboarding script, a CI setup step, or a dotfile automation will hit this. The pattern of "runs fine locally, hangs in CI" is a well-known source of frustration.

**Recommendations:**
1. Add `--spec <path>` and optionally `--name <slug>` arguments to the `Init` subcommand. When these flags are provided, skip all prompts and write the config directly.
2. Add a TTY check using `is-terminal` before invoking any `read_line`. If stdin is not a TTY and the required flags were not provided, fail immediately with: `error: --spec is required when running non-interactively\n       run 'phyllotaxis init --spec path/to/spec.yaml'`
3. The `run_add_spec` path (lines 175–271) has the same issue and needs the same treatment.

---

### Finding 8: `run_add_spec` in `init.rs` manually manipulates YAML by string operations

**Severity:** Medium
**Location:** `src/commands/init.rs:228–248`
**Issue:** When adding a second spec to an existing config that already has a `specs:` block, the code manipulates the YAML file as raw text using string line iteration:
```rust
let last_specs_idx = lines
    .iter()
    .enumerate()
    .filter(|(_, l)| l.starts_with("  ") && !l.trim().is_empty())
    .map(|(i, _)| i)
    .last();
```
This heuristic — "find the last line that starts with two spaces" — will silently corrupt the config if the YAML has any comment lines, extra blank lines, or if the `variables` block is indented (which it is, per the `Config` struct). For example, a config with `variables:\n  tenant: acme-corp\n` would have the new spec entry inserted inside the `variables` block.

**Why it matters:** This is a silent data corruption bug in the init path. A user adding a second spec could corrupt their config in a way that is not immediately obvious, and debugging it requires understanding the YAML format and the insertion heuristic.

**Recommendations:**
1. Parse the existing config using `serde_yaml`, modify the deserialized `Config` struct, and re-serialize it. This is the only correct approach for YAML manipulation. The `serde_yaml` crate is already a dependency.
2. Show the user a preview of what will be written before writing it (confirm-before-write principle from the research report).

---

### Finding 9: `init` does not confirm before writing and does not show "what now"

**Severity:** Medium
**Location:** `src/commands/init.rs:161–172`
**Issue:** After the user selects a spec, the config is written immediately with no confirmation step:
```rust
std::fs::write(&config_path, content).expect("failed to write .phyllotaxis.yaml");
eprintln!("Initialized. Run `phyllotaxis` to see your API overview.");
```
The research report's first-run design principles include: "Confirm before writing — show a summary of what will be written and to where before creating any files." The "what now" message (`"Initialized. Run phyllotaxis..."`) is present but minimal — it does not mention the config file path that was written, which is useful to know if the user wants to inspect or modify it.

**Why it matters:** Writing files without a confirmation step can surprise users who made an accidental selection or mistyped a path. The confirmation step is cheap to add and meaningful as a safety net.

**Recommendations:**
1. Before writing, print what will be written: `"Will write to /path/to/.phyllotaxis.yaml:\n  spec: ./my-spec.yaml\nProceed? [Y/n]"` (gated on TTY check — skip in non-interactive mode).
2. After writing, include the config file path in the success message: `"Initialized: wrote .phyllotaxis.yaml\n  spec: ./my-spec.yaml\n\nNext: phyllotaxis"`

---

### Finding 10: `--expand` is a global flag but only makes sense on `resources` and `schemas`

**Severity:** Medium
**Location:** `src/main.rs:21–23`
**Issue:** `--expand` is defined at the top-level `Cli` struct with `#[arg(global = true)]`. This makes it appear in the help text for every subcommand, including `auth`, `search`, and `init`, where it has no effect. A user reading `phyllotaxis auth --help` will see `--expand` listed as an option and may try it, getting no indication that it was silently ignored.

**Why it matters:** Global flags are appropriate for flags that apply universally (`--spec`, `--json`). `--expand` is semantically scoped to schema traversal. Advertising it globally misleads users and creates silent no-ops.

**Recommendations:**
1. Move `--expand` out of the global `Cli` struct and into the `Resources` and `Schemas` subcommand variants in the `Commands` enum. This requires passing `expand` from the match arms in `main.rs` — the logic is already correct, it just needs to be scoped.
2. If `--expand` is desired on the default (no-subcommand) overview path too, it can still be defined there explicitly.

---

### Finding 11: `render_param_section` in `text.rs` always prints a section header, even when empty; path/query sections always shown

**Severity:** Low
**Location:** `src/render/text.rs:173–201`
**Issue:** `render_param_section` is called for path and query parameters unconditionally for all endpoints, even those with no parameters of that type:
```rust
render_param_section(&mut out, "Path Parameters", &path_params);
render_param_section(&mut out, "Query Parameters", &query_params);
```
The function then prints `"(none)"` as the content. Header parameters are conditional (`if !header_params.is_empty()`). This means endpoints without path or query parameters will always print two sections that say "(none)":
```
Path Parameters:
  (none)

Query Parameters:
  (none)
```
This is visual noise that does not help the user.

**Why it matters:** For simple endpoints (e.g., a DELETE with no parameters), the output is dominated by empty sections rather than the substantive information (responses, security). Progressive disclosure means showing only what exists.

**Recommendations:**
1. Apply the same guard used for header parameters to path and query sections: only call `render_param_section` if the corresponding slice is non-empty.
2. If it is genuinely useful to show "no parameters at all" when an endpoint has zero parameters of any kind, add a single line `"Parameters: (none)\n"` instead of separate empty sections per location.

---

### Finding 12: `render_overview` JSON output embeds a `commands` field with suggested commands

**Severity:** Low
**Location:** `src/render/json.rs:22–28`, `src/render/json.rs:55–61`
**Issue:** The JSON overview output includes a `commands` field:
```json
"commands": {
  "resources": "phyllotaxis resources",
  "schemas": "phyllotaxis schemas",
  "auth": "phyllotaxis auth",
  "search": "phyllotaxis search"
}
```
This is human guidance embedded in machine-readable output. A script consuming `phyllotaxis --json` to extract API metadata (title, servers, auth schemes, resource count) does not need the tool to tell it what commands to run next. Similarly, `drill_deeper` fields appear in `render_resource_list`, `render_resource_detail`, and `render_schema_detail` JSON outputs.

**Why it matters:** The research report states that JSON output should be "suitable for direct consumption by `jq`, scripts, and other tooling" with "stable keys across versions." Embedding navigational suggestions as JSON keys creates keys that must be documented as non-data and creates noise for consumers that need to filter them out.

**Recommendations:**
1. Remove the `commands`, `drill_deeper`, and `total` wrapper fields from all JSON output. The data itself (title, servers, auth, resource list, schema list) is sufficient.
2. These navigational hints are well-suited to the text renderer (where they already appear) and have no place in the JSON renderer.

---

### Finding 13: No shell completion support

**Severity:** Low
**Location:** `Cargo.toml`, `src/main.rs`
**Issue:** There is no `completions` subcommand and `clap_complete` is not a dependency. For a tool used repeatedly to explore different specs, tab-completion of subcommand names (`resources`, `schemas`, `auth`, `search`, `init`) and global flags (`--spec`, `--json`, `--expand`) would meaningfully reduce friction.

**Why it matters:** The research report notes: "Shell completions via `clap_complete` would significantly improve day-to-day ergonomics for a tool used repeatedly against different specs. Adding a `completions` subcommand is low-effort relative to the UX gain."

**Recommendations:**
1. Add `clap_complete = "4"` to `[dependencies]`.
2. Add a `Completions { shell: clap_complete::Shell }` variant to the `Commands` enum.
3. In `main.rs`, handle it by calling `clap_complete::generate(shell, &mut Cli::command(), "phyllotaxis", &mut std::io::stdout())` and exit 0.
4. Document usage in `--help`: `phyllotaxis completions bash > ~/.local/share/bash-completion/completions/phyllotaxis`

---

### Finding 14: No `--quiet`/`-q` flag and no verbosity levels

**Severity:** Low
**Location:** `src/main.rs:9–26` (Cli struct)
**Issue:** There is no `--quiet` or `--verbose` flag. The research report and clig.dev both describe the standard verbosity convention (`-q` for errors only, default for results, `-v` for informational, `-vv` for debug). For phyllotaxis, a `--quiet` flag would suppress the "Drill deeper:" sections in text output and reduce output to the essential data only. This is useful when piping text output through `grep`.

**Why it matters:** Without `-q`, text output always includes navigational hints ("Drill deeper:", "Commands:") that add noise when text output is piped. Users who know the tool do not need these hints every time.

**Recommendations:**
1. Add `clap-verbosity-flag = "2"` (or a manual `--quiet`/`-v` pair) to the `Cli` struct.
2. In text renderers, gate "Drill deeper:" and "Commands:" sections on `!quiet`. This is a small change to the text renderer functions, passing a `quiet: bool` parameter.

---

### Finding 15: No `human-panic` for panic handling

**Severity:** Low
**Location:** `src/main.rs:60` (`fn main()`)
**Issue:** If phyllotaxis panics (e.g., an unexpected `None` unwrap during spec parsing of a malformed but syntactically valid YAML), the user sees a raw Rust panic message with a stack trace, or the message `thread 'main' panicked at 'called Option::None.unwrap()'`. There are several `expect()` calls that could panic in edge cases (e.g., `main.rs:62`: `expect("cannot determine current directory")`, `init.rs:169`: `expect("failed to write .phyllotaxis.yaml")`).

**Why it matters:** The research report: "Panics should never reach users as raw stack traces." A raw panic message is confusing and unhelpful. `human-panic` intercepts panics and replaces them with a friendly message that includes a path to a crash report.

**Recommendations:**
1. Add `human-panic = "2"` to `[dependencies]`.
2. Add `human_panic::setup_panic!()` as the first line of `main()`.
3. Consider converting the `expect()` calls in init.rs (file write failure) to graceful `eprintln!` + `std::process::exit(1)` — a failed config write is a user-visible error, not a panic scenario.

---

### Finding 16: No `--no-color` flag and no `NO_COLOR` environment variable respect

**Severity:** Low
**Location:** `src/main.rs:9–26` (Cli struct), `Cargo.toml`
**Issue:** Related to Finding 6 (no color at all currently), but distinct: even if color is added, there is currently no `--no-color` flag and no `NO_COLOR` environment variable handling. The research report: "The `NO_COLOR` environment variable is the ecosystem-wide opt-out signal. If `NO_COLOR` is set to any non-empty value, all ANSI color output must be suppressed regardless of TTY state."

**Why it matters:** When color is added, the absence of `NO_COLOR` support will immediately break users running phyllotaxis in terminals that support ANSI but have a color-unfriendly theme or who are piping to a tool that does not strip escape codes.

**Recommendations:**
1. Use `owo-colors`'s `if_supports_color` method, which handles `NO_COLOR` and `TERM=dumb` automatically. This means no manual `NO_COLOR` env var check is needed.
2. Add `--no-color` as a boolean flag in the `Cli` struct that, when set, forces `owo-colors`'s stream support off globally (achievable by setting `NO_COLOR=1` in the process environment early in `main()`).

---

### Finding 17: Missing help examples in subcommand definitions

**Severity:** Informational
**Location:** `src/main.rs:29–53`
**Issue:** The `Commands` enum uses doc comments (`///`) for subcommand descriptions, but none of the commands include usage examples in their help text. clap's derive API supports long doc comments with embedded examples. Currently, `phyllotaxis resources --help` shows only the argument list with no examples. The research report: "Include at least one concrete example in each subcommand's help text."

**Why it matters:** Examples are the fastest way to orient a new user. A user looking at `phyllotaxis search --help` has no indication that `phyllotaxis search "create user"` or `phyllotaxis search pet --json` are valid invocations.

**Recommendations:**
1. Expand the doc comments on each `Commands` variant to include an `Examples:` block. clap renders multi-line doc comments as part of long help (`--help`):
   ```rust
   /// Search across all endpoints and schemas.
   ///
   /// Searches path names, summaries, descriptions, parameter names, and schema names.
   ///
   /// Examples:
   ///   phyllotaxis search pet
   ///   phyllotaxis search "create user" --json
   Search {
       /// Search term (case-insensitive substring match)
       term: String,
   }
   ```
2. Do the same for `Resources`, `Schemas`, `Auth`, and `Init`.

---

### Finding 18: `run_add_spec` migration instructions print config YAML to stderr without the config file name until the end

**Severity:** Informational
**Location:** `src/commands/init.rs:254–269`
**Issue:** When the existing config uses the single-spec format and the user tries to add a second spec, the code prints migration instructions as raw YAML fragments to stderr:
```rust
eprintln!("  specs:");
eprintln!("    default: {}", old_spec);
eprintln!("    {}: {}", name, relative);
eprintln!("  default: default\n");
```
The user must manually edit the config. The config file path is mentioned at the start of the function (`eprintln!("Config already exists at {}.", config_path.display())`), but the YAML to copy is emitted without any framing — no "Copy and paste the following into your config file:" header — which makes the output harder to follow.

**Why it matters:** This is a friction point in the multi-spec onboarding flow. The error message does contain the correct information but the presentation could be clearer.

**Recommendations:**
1. Print a clear framing header before the YAML block: `"To add multiple specs, replace the contents of {} with:"` followed by the full YAML.
2. Consider whether this migration can be automated (parse and rewrite as in Finding 8's recommendation) rather than requiring the user to manually edit the file.

---

## Prioritized Action List

The following items are ordered by impact on real-world usability.

1. **[High] Fix JSON list output to return bare arrays** (Finding 1) — breaks `jq` usage, the most common automation pattern for a tool like this.

2. **[High] Emit JSON errors when `--json` is active** (Finding 3) — makes the tool fully scriptable; currently scripts must text-parse stderr to detect errors.

3. **[High] Compact JSON when piped, pretty when TTY** (Finding 2) — requires adding `is-terminal`; unlocks correct behavior for both interactive and automated use.

4. **[Medium] Add non-interactive flag path to `init`** (Finding 7) — blocks use in CI and onboarding scripts; requires adding `--spec` argument to the `Init` variant.

5. **[Medium] Replace edit-distance suggestion logic** (Finding 5) — substring matching misses the most common typo patterns; requires adding `strsim` to dependencies.

6. **[Medium] Fix YAML manipulation in `run_add_spec`** (Finding 8) — silent data corruption risk; replace string heuristic with `serde_yaml` parse-modify-reserialize.

7. **[Medium] Move `--expand` to subcommand scope** (Finding 10) — currently advertised on `auth`, `search`, and `init` where it does nothing.

8. **[Medium] Add color with TTY detection** (Finding 6, Finding 16) — deprecated/alpha markers are easy to miss without visual distinction; requires `owo-colors` + `is-terminal`.

9. **[Medium] Change "Did you mean" phrasing** (Finding 4) — polish fix aligned with Rust ecosystem convention; a one-line change per occurrence.

10. **[Medium] Add confirm-before-write and improved "what now" to `init`** (Finding 9) — low-risk safety improvement for first-run experience.

11. **[Low] Remove empty parameter sections from endpoint detail** (Finding 11) — visual noise reduction; trivial guard condition change.

12. **[Low] Remove `commands`/`drill_deeper`/`total` from JSON output** (Finding 12) — prose guidance does not belong in machine-readable output.

13. **[Low] Add shell completions via `clap_complete`** (Finding 13) — meaningful ergonomic improvement for repeat users; low implementation effort.

14. **[Low] Add `--quiet`/`-q` flag** (Finding 14) — suppresses navigational hints when text output is piped to `grep` or other tools.

15. **[Low] Add `human-panic`** (Finding 15) — replaces raw panic output with a friendly crash report; one line in `main()`.

16. **[Informational] Add usage examples to subcommand help text** (Finding 17) — improves discoverability for new users; doc comment edits only.

17. **[Informational] Improve migration instructions framing in `run_add_spec`** (Finding 18) — clarity improvement for multi-spec onboarding.
