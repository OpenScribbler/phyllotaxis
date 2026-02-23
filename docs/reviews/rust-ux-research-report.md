# Rust CLI UX Research Report

**Subject:** UX best practices for Rust CLI applications, with focus on phyllotaxis
**Date:** 2026-02-21
**Scope:** Output design, error messages, help text, command structure, interactivity, machine readability, progress feedback, first-run experience, Rust crate ecosystem, POSIX conventions

---

## 1. Output Design

### The Fundamental Rule: stdout for Data, stderr for Everything Else

All content the user is meant to consume goes to `stdout`. All diagnostic information — errors, warnings, progress messages, status updates — goes to `stderr`. This separation is not convention; it is a functional requirement for composability. When a user pipes `phyllotaxis resources | jq '.[0]'`, any error or status message on `stdout` breaks the downstream consumer. This rule applies equally to `--json` mode and human-readable mode.

The corollary: `--json` output must be clean enough to pass directly to `jq` without preprocessing. No preamble text, no trailing notes, no mixing of structured and prose output.

Sources: [clig.dev](https://clig.dev/), [Tips on Adding JSON Output to Your CLI App](https://blog.kellybrazil.com/2021/12/03/tips-on-adding-json-output-to-your-cli-app/)

### TTY Detection and Adaptive Output

Output behavior should adapt based on whether stdout is a terminal (TTY) or a pipe/redirect. When not a TTY:

- Disable all color and ANSI escape sequences
- Disable progress spinners and bars (they corrupt log files and CI output)
- Consider disabling box-drawing characters and other Unicode decoration if targeting minimal environments

The standard detection mechanism in Rust is the `is-terminal` crate, which wraps `libc`'s `isatty()` in a safe, cross-platform API. `owo-colors` integrates with this via its `if_supports_color` method. The `supports-color` crate goes further, detecting whether the terminal supports 16 colors, 256 colors, or true color — useful if you want to degrade gracefully rather than switching color off entirely.

The `NO_COLOR` environment variable is the ecosystem-wide opt-out signal. If `NO_COLOR` is set to any non-empty value, all ANSI color output must be suppressed regardless of TTY state. The `TERM=dumb` case should also disable color. `owo-colors` handles both automatically when using `if_supports_color`.

For phyllotaxis specifically: color is appropriate for labeling (`Resource:`, `Schema:`, `Error:`) and for `--expand` output that recurses into nested schemas, where indentation depth alone may be insufficient to convey structure. Color is not appropriate in `--json` mode, and the tool should not emit it when piped.

### Progressive Disclosure as an Output Principle

Progressive disclosure in CLI output means showing the minimal useful information by default and requiring explicit flags or subcommands to drill deeper. This directly matches phyllotaxis's stated purpose. A concrete implementation strategy:

- `phyllotaxis resources` — lists resource names only (one per line, or a compact table)
- `phyllotaxis resources --verbose` — includes HTTP methods, paths, and short descriptions
- `phyllotaxis resources <name>` — full detail for a single resource
- `phyllotaxis resources <name> --expand` — recursively inlines schema references

Each level outputs only what was asked for. The default level should answer "what exists?" without overwhelming with the full spec.

### Text Formatting: Tables, Columns, and Line Width

For structured output, `comfy-table` is the preferred Rust crate. It handles automatic column width calculation, terminal-width-aware layout, and graceful wrapping. `tabled` is a viable alternative with more customization options but greater complexity.

Key formatting rules:
- Respect the terminal width (read `COLUMNS` env var or use `terminal_size` crate)
- Left-align text fields; right-align numeric fields
- Avoid deeply nested indentation (more than 3 levels becomes hard to scan)
- Prefer one-record-per-line when output will be processed by `grep` or `awk`

Sources: [clig.dev – Output section](https://clig.dev/), [comfy-table on lib.rs](https://lib.rs/crates/comfy-table)

---

## 2. Error Messages

### The Three Parts of a Good Error Message

Every error message should contain three components:

1. **What went wrong** — stated concisely, in plain language, not as a stack trace
2. **Why it happened** — context that helps the user understand the cause
3. **What to do next** — an actionable suggestion, a corrected command, or a path to documentation

Example of a poor error message:
```
Error: Not found
```

Example of a good error message:
```
error: schema 'UserRespons' not found in spec

  tip: a schema with a similar name exists: 'UserResponse'
       run `phyllotaxis schemas UserResponse` to view it
```

The Rust compiler's diagnostic format (`error:`, `note:`, `help:`, `tip:`) is a well-understood model worth borrowing. `miette` provides a full implementation of this pattern including source spans, color-coded severity levels, and suggestions.

### "Did You Mean" — Phrasing and Implementation

The Rust compiler development guide explicitly recommends against the phrasing "did you mean" because it frames a statement of fact as a question, which can feel uncertain. The preferred alternative is a declarative statement: "a resource with a similar name exists: `X`" or "similar name found: `X`".

Implementation: the `strsim` crate provides Levenshtein, OSA (Optimal String Alignment), Jaro-Winkler, and other metrics. It is the crate already used internally by `clap` for its own argument suggestions. For phyllotaxis's resource and schema name matching, Levenshtein or OSA distance is appropriate. A threshold of 3 edits is a reasonable cutoff for short identifiers; normalize by string length for longer names.

```
// pseudocode: suggest closest match
let candidates = spec.resource_names();
let best = candidates.iter()
    .map(|name| (name, strsim::levenshtein(input, name)))
    .filter(|(_, dist)| *dist <= 3)
    .min_by_key(|(_, dist)| *dist);
```

Sources: [Rust Compiler Diagnostics Guide](https://rustc-dev-guide.rust-lang.org/diagnostics.html), [strsim on crates.io](https://crates.io/crates/strsim)

### Error Verbosity: miette vs anyhow vs eyre

For application-level error handling in Rust CLIs, the options form a spectrum:

- **`anyhow`** — the pragmatic baseline; wraps any `std::error::Error`, supports context chaining with `.context()`, outputs a clean error chain. Best for internal errors that users rarely see.
- **`eyre` + `color-eyre`** — a fork of `anyhow` with customizable error handlers. `color-eyre` adds colored output, backtraces on demand (via `RUST_BACKTRACE=1`), and suggestion hooks. Good middle ground for user-facing errors.
- **`miette`** — the richest option; provides compiler-style diagnostics with source code snippets, labeled spans, and multiple severity levels (`error`, `warning`, `advice`). Designed specifically for errors that users need to understand and act on. Most appropriate for user-input validation errors (like "schema not found") where pointing to the specific problematic input adds value.

For phyllotaxis: `anyhow` for internal/IO errors, `miette` for user-facing validation errors (unknown resource/schema, malformed spec reference, missing spec file). The combination is intentional and common in the ecosystem.

### Panics: human-panic

Panics should never reach users as raw stack traces. The `human-panic` crate intercepts panics and replaces the output with a friendly message:

```
Well, this is embarrassing.

phyllotaxis had a problem and crashed. To help us diagnose the problem,
you can send us a crash report.

We have generated a report file at '/tmp/report-abc123.toml'.
Submit an issue at: https://github.com/...
```

One line in `main()` enables it: `human_panic::setup_panic!()`.

Sources: [human-panic on GitHub](https://github.com/rust-cli/human-panic), [Rust CLI Book – Nicer error reporting](https://rust-cli.github.io/book/tutorial/errors.html)

---

## 3. Help Text and Documentation

### --help vs -h

Both `-h` and `--help` should work. In `clap`'s derive API, `-h` by default shows a short summary and `--help` shows the full help. This is appropriate behavior — power users who know the flags get the brief version, and users who are confused get comprehensive text.

Help text best practices with `clap` derive:
- Doc comments (`///`) on structs and fields become help text automatically
- Use `.help_expected(true)` in debug builds to enforce that every argument has help text
- Lead with the most common use case, not the complete specification
- Include at least one concrete example in each subcommand's help text

```rust
/// List resources defined in the OpenAPI spec.
///
/// By default, shows resource names and a brief description.
/// Use --verbose to include HTTP methods and paths.
///
/// Examples:
///   phyllotaxis resources
///   phyllotaxis resources users --expand
#[derive(Parser)]
struct ResourcesCmd {
    /// Resource name to inspect (optional; lists all if omitted)
    name: Option<String>,

    /// Recursively inline all $ref schemas
    #[arg(long)]
    expand: bool,
}
```

### Argument Naming Conventions

- Flags use `--kebab-case` (clap handles the mapping from Rust's `snake_case` automatically)
- Single-character short flags (`-j`, `-v`, `-q`) for the most frequent options
- Positional arguments only for the single most obvious input; everything else is a named flag
- Boolean flags are named for what enabling them does (`--json`, `--expand`, `--verbose`), never negations as the primary form
- Override flags pair with their disable counterpart only when there is a genuine need to override a config default (e.g., `--no-color` pairs with always-on color in a config file)

### Global vs. Subcommand-Local Flags

Flags that apply to all subcommands (`--spec`, `--json`) should be defined at the top-level `App` struct and marked `#[arg(global = true)]`. This allows them to appear before or after the subcommand name:

```
phyllotaxis --json resources
phyllotaxis resources --json   # also works with global = true
```

Subcommand-specific flags (`--expand` on `resources`, `--schemas` on `search`) belong in their own structs.

Sources: [Rain's Rust CLI Recommendations – Handling Arguments](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html), [clap docs](https://docs.rs/clap/latest/clap/)

---

## 4. Command Naming and Structure

### Nouns vs. Verbs

A common debate: should subcommands be nouns (`resources`, `schemas`) or verb-noun pairs (`list-resources`, `get-schema`)?

For inspection/read-only tools, nouns are more natural and match established precedent (`kubectl get`, `gh pr`, `docker ps`). The subcommand name identifies what domain is being queried; the mode of query is implied (or controlled by positional/named arguments). This matches phyllotaxis's current design.

For tools that perform mutations, verb-noun pairs are clearer because the action is not implied. Since phyllotaxis is read-only, noun-based commands are appropriate.

### Subcommand Organization Principles

- Keep the command namespace flat when the total number of subcommands is small (fewer than ~8)
- Group related subcommands under a parent only when they form a coherent domain (e.g., `phyllotaxis auth list`, `phyllotaxis auth show` rather than `phyllotaxis auth-list`, `phyllotaxis auth-show`)
- Avoid catch-all or default subcommands — if `phyllotaxis foo` falls through to some default, it blocks future addition of a `foo` subcommand
- Never abbreviate subcommand names in the public interface; abbreviations can be aliases but not the canonical name

### Naming Conventions for Flags

| Pattern | Example | Use |
|---|---|---|
| `--output-format` | `--output-format json` | When multiple output types exist |
| `--json` | (boolean) | Acceptable shorthand when only one machine format is offered |
| `--verbose` / `-v` | | Log level increase |
| `--quiet` / `-q` | | Suppress non-error output |
| `--spec` | `--spec path/to/spec.yaml` | File path override |
| `--no-color` | (boolean) | Explicit color disable |

Sources: [clig.dev – Subcommands](https://clig.dev/), [Atlassian CLI Design Principles](https://www.atlassian.com/blog/it-teams/10-design-principles-for-delightful-clis)

---

## 5. Interactivity

### The Rule: Flag-Equivalent for Every Prompt

Every interactive prompt must have a non-interactive flag equivalent. Prompts are for convenience in TTY sessions; flags are for automation. A user running phyllotaxis in a CI pipeline, a Makefile, or a shell script will pass flags — if those flags don't exist, the tool is unusable in automation.

For `init`, this means every prompt (spec file path, default format preference, etc.) must have a corresponding `--spec-path`, `--format`, etc. flag.

### TTY Detection for Prompt Gating

Only show interactive prompts when `stdin` is a TTY. Use the `is-terminal` crate to check. If stdin is not a TTY and a required value was not provided as a flag, fail with a clear error:

```
error: --spec is required when running non-interactively
       run `phyllotaxis init --spec path/to/spec.yaml` or run interactively in a terminal
```

### Interactive Prompt Crate Selection

For the `init` command's interactive flow, the four main options are:

- **`dialoguer`** — mature, widely used, supports themes, input validation, select lists, confirmation prompts. Good default choice.
- **`inquire`** — feature-rich with autocomplete suggestions and custom validators. Appropriate if init involves selecting from a list of discovered spec files.
- **`cliclack`** — modern visual style, theming support. Good for a polished first-run experience.
- **`promptly`** — minimal; best for simple yes/no or single-value prompts with minimal overhead.

For phyllotaxis, `dialoguer` or `cliclack` is recommended. The init flow is bounded (finite questions), doesn't require autocomplete, and benefits from a clean visual style that signals "this is a setup wizard, not an error."

### Ctrl-C Handling

Interactive sessions must exit cleanly on Ctrl-C without printing a stack trace or panic message. Check for `Interrupted` errors from dialoguer/inquire and exit with code 130 (the POSIX convention for termination by SIGINT: `128 + 2`).

Sources: [clig.dev – Interactivity](https://clig.dev/), [Comparison of Rust CLI Prompts](https://fadeevab.com/comparison-of-rust-cli-prompts/), [Ubuntu CLI Guidelines – Interactive Prompts](https://discourse.ubuntu.com/t/interactive-prompts/18881)

---

## 6. Machine Readability

### --json Flag Design

The `--json` flag should produce output suitable for direct consumption by `jq`, scripts, and other tooling. Specific rules:

- Output is a single JSON value — either an object or an array — never a stream of prose with embedded JSON
- Keys use `snake_case` consistently (or `camelCase` if documented; snake_case is more idiomatic with Rust's `serde` defaults)
- No omitted fields based on nil/empty values; include them as `null` or `[]` so consumers can rely on stable keys
- Numbers that exceed JavaScript's safe integer range should be strings
- Keys should be stable across versions; adding fields is safe, removing or renaming is a breaking change

### JSON Structure for List vs. Detail

Commands that return lists should return a JSON array at the top level when `--json` is used. Commands that return a single item should return a JSON object. Do not wrap in an envelope object (`{"data": [...]}`) unless metadata (like pagination) must accompany the result.

```json
// phyllotaxis resources --json
[
  { "name": "users", "methods": ["GET", "POST"], "path": "/users" },
  { "name": "orders", "methods": ["GET"], "path": "/orders" }
]

// phyllotaxis resources users --json
{ "name": "users", "methods": ["GET", "POST"], "path": "/users", "description": "..." }
```

### JSON Errors

When `--json` is active, errors should also be JSON rather than plain text on stderr. This allows scripts to parse error conditions without separate stderr handling:

```json
// on stderr, exit code 1
{ "error": "schema_not_found", "message": "Schema 'UserRespons' not found", "suggestion": "UserResponse" }
```

This is a deliberate design choice — tools like the AWS CLI and GitHub CLI implement this pattern. It makes the tool fully scriptable.

### Compact vs. Pretty JSON

Default to compact (no extra whitespace) when piped; use pretty-printed (two-space indent) when stdout is a TTY and `--json` is explicitly requested. Users who need formatted JSON can pipe through `jq .`. The `serde_json` crate supports both via `to_string()` (compact) and `to_string_pretty()`.

Sources: [Tips on Adding JSON Output to Your CLI App](https://blog.kellybrazil.com/2021/12/03/tips-on-adding-json-output-to-your-cli-app/), [clig.dev – Output](https://clig.dev/)

---

## 7. Feedback and Progress

### When to Show Progress

Show a progress indicator for any operation that takes more than ~100ms to produce its first output. For phyllotaxis, the main candidate is spec file parsing — loading and parsing a large OpenAPI spec (hundreds of endpoints, deep schema trees) can take perceptible time.

Rules:
- Spinners for operations of unknown duration
- Progress bars for operations where total work is known
- No progress indicators when stdout is not a TTY (spinners corrupt logs)
- No progress indicators in `--json` mode

### indicatif

`indicatif` is the standard Rust crate for progress bars and spinners. It integrates with the `console` crate for formatting and correctly handles TTY detection. Key usage pattern:

```rust
let pb = indicatif::ProgressBar::new_spinner();
pb.set_message("Loading spec...");
pb.enable_steady_tick(Duration::from_millis(80));
// ... do work ...
pb.finish_and_clear();
```

`finish_and_clear()` removes the spinner line, leaving the terminal clean for subsequent output. This is preferable to `finish_with_message()` when the next output provides adequate confirmation.

### Verbosity Levels

Follow the de-facto convention:

| Flag | Level | Output |
|---|---|---|
| `-q` / `--quiet` | Quiet | Errors only |
| (default) | Normal | Errors + key results |
| `-v` / `--verbose` | Verbose | + informational messages |
| `-vv` | Debug | + debug-level output |

`clap-verbosity-flag` provides a ready-made implementation of this pattern that integrates with the `log` crate.

In `--json` mode, `-q` suppresses stderr diagnostic messages but does not suppress the JSON output itself (which is the primary content).

Sources: [Evil Martians – CLI UX Progress Displays](https://evilmartians.com/chronicles/cli-ux-best-practices-3-patterns-for-improving-progress-displays), [indicatif on crates.io](https://crates.io/crates/indicatif), [Ubuntu CLI Guidelines – Verbosity Levels](https://discourse.ubuntu.com/t/cli-verbosity-levels/26973)

---

## 8. First-Run Experience

### The init Command

The `init` command is the user's first interaction with phyllotaxis as a configured tool. It should feel guided and safe:

1. **Ask, don't assume** — prompt for the spec file path rather than guessing; but offer a sensible default if a `openapi.yaml` or `swagger.json` is found in the current directory
2. **Confirm before writing** — show a summary of what will be written and to where before creating any files
3. **Be idempotent** — running `init` a second time should not corrupt the existing config; either update it or ask whether to overwrite
4. **Give a "what now" prompt** — after `init` completes, print the next command the user should run (e.g., `phyllotaxis resources`)

### Config File Location

Follow XDG Base Directory conventions on Linux/macOS:
- Config file: `$XDG_CONFIG_HOME/phyllotaxis/config.toml` (defaults to `~/.config/phyllotaxis/config.toml`)
- Use the `dirs` crate for cross-platform path resolution
- Use TOML format (human-readable, common in the Rust ecosystem)
- Support a project-local config (e.g., `.phyllotaxis.toml` in the current directory) that overrides the user-level config

### Config Precedence

In strict priority order (highest to lowest):

1. CLI flags (`--spec path/to/spec.yaml`)
2. Environment variables (`PHYLLOTAXIS_SPEC`)
3. Project-local config (`.phyllotaxis.toml` in current directory)
4. User config (`~/.config/phyllotaxis/config.toml`)
5. Built-in defaults

This lets users override any setting for a single invocation without modifying files, and supports per-project spec file configuration without polluting global state.

### What init Should Produce

A minimal first-run config might look like:

```toml
# ~/.config/phyllotaxis/config.toml
spec = "path/to/openapi.yaml"
default_format = "text"   # or "json"
```

Keep it minimal. Do not generate config keys for every possible option on first run — that creates friction if option names change and creates false impressions that all keys are required.

Sources: [Rain's Rust CLI Recommendations – Configuration](https://rust-cli-recommendations.sunshowers.io/configuration.html), [XDG Base Directory and Rust](https://blog.liw.fi/posts/2021/02/14/xdg-base-dirs-rust/), [dirs on crates.io](https://crates.io/crates/dirs)

---

## 9. Rust-Specific UX Crates

### Curated Crate List

| Crate | Purpose | Notes |
|---|---|---|
| `clap` (derive) | Argument parsing | Industry standard; derive API is cleanest |
| `clap_complete` | Shell completions | Generates bash/zsh/fish/elvish scripts |
| `owo-colors` | Terminal color | Zero-alloc, `NO_COLOR`-aware, no global state |
| `indicatif` | Progress bars/spinners | TTY-aware; use `finish_and_clear()` |
| `dialoguer` or `cliclack` | Interactive prompts | For `init` wizard |
| `comfy-table` | Terminal tables | Auto-width, wrapping |
| `miette` | Rich diagnostics | Compiler-style errors with source spans |
| `anyhow` | Error propagation | `.context()` for error chaining |
| `human-panic` | Panic handler | One-line `setup_panic!()` |
| `strsim` | String similarity | Powers "similar name" suggestions |
| `is-terminal` | TTY detection | Modern replacement for `atty` |
| `supports-color` | Color capability detection | 16/256/truecolor support levels |
| `dirs` | XDG/platform config paths | Cross-platform; most actively maintained |

### Crates to Avoid

- `colored` — does not support `NO_COLOR` properly; use `owo-colors` instead
- `termcolor` — targets deprecated Windows Console APIs; `owo-colors` + `enable-ansi-support` is preferred
- `atty` — unmaintained; use `is-terminal` instead
- Raw `println!` for errors — always use `eprintln!` for error output

### Shell Completions

`clap_complete` generates completion scripts at runtime (via a `--completions <shell>` flag) or at build time (via `build.rs`). Runtime generation is simpler to implement and keeps completions in sync with the installed binary version automatically. Adding a `completions` subcommand is the conventional exposure point:

```
phyllotaxis completions bash > ~/.local/share/bash-completion/completions/phyllotaxis
phyllotaxis completions zsh > ~/.zfunc/_phyllotaxis
```

Sources: [owo-colors on crates.io](https://crates.io/crates/owo-colors), [Managing Colors in Rust](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html), [10 Essential Rust Crates for CLI Tools](https://elitedev.in/rust/10-essential-rust-crates-for-building-professional/), [clap_complete on crates.io](https://crates.io/crates/clap_complete)

---

## 10. POSIX Conventions

### Exit Codes

Zero means success. Non-zero means failure. Everything downstream (shell scripts, CI pipelines, make targets) depends on this contract. Specific mappings:

| Exit Code | Meaning | When to Use |
|---|---|---|
| `0` | Success | Command completed as expected |
| `1` | General error | Catch-all for errors not listed below |
| `2` | Misuse / bad arguments | Invalid flags, unknown subcommand |
| `64` (`EX_USAGE`) | Usage error | Wrong number of arguments |
| `65` (`EX_DATAERR`) | Data format error | Malformed spec file |
| `66` (`EX_NOINPUT`) | Input not found | Spec file not found |
| `78` (`EX_CONFIG`) | Config error | Invalid config file |
| `130` | Interrupted | SIGINT (Ctrl-C); = 128 + 2 |

The `exitcode` crate provides named constants for the `sysexits.h` codes (64–78), which are the most commonly used semantic exit codes beyond 0/1. Rust's default panic exit code is 101, which `human-panic` preserves.

### stdin Conventions

If phyllotaxis ever needs to accept piped input (e.g., piping a spec through stdin instead of a file path), support the `-` convention: `--spec -` reads from stdin. Check `is_terminal::IsTerminal::is_terminal(&std::io::stdin())` to detect whether stdin is a pipe before attempting to read it.

If stdin is a TTY and your command expects piped input, print a helpful message and exit rather than hanging.

### Signal Handling

At minimum, ensure Ctrl-C exits cleanly. For interactive prompts, `dialoguer` and `cliclack` handle this internally. For long-running operations using `indicatif`, the crate's drop implementation ensures the progress bar is cleaned up on exit, including on Ctrl-C.

### Environment Variables

Phyllotaxis should respect these standard variables:
- `NO_COLOR` — disable all ANSI color output
- `TERM` — check for `dumb` to disable color and advanced formatting
- `COLUMNS` — use for terminal width if querying the terminal directly is not possible

Any application-specific environment variables should be prefixed (`PHYLLOTAXIS_SPEC`, `PHYLLOTAXIS_FORMAT`) to avoid collision with other tools.

Sources: [Rust CLI Book – Exit Codes](https://rust-cli.github.io/book/in-depth/exit-code.html), [Standard Exit Status Codes in Linux (Baeldung)](https://www.baeldung.com/linux/status-codes), [clig.dev – Exit Codes](https://clig.dev/)

---

## 11. Synthesis: Implications for phyllotaxis

Drawing the research together into direct observations:

**Output pipeline integrity** is the highest-priority concern. The `--json` flag and the `--spec` flag together make phyllotaxis a building block for larger workflows. Contaminating `stdout` with any non-JSON output in `--json` mode, or printing progress to `stdout` instead of `stderr`, immediately breaks those workflows. This must be treated as a correctness issue, not a style issue.

**The "did you mean" phrasing** should be replaced with declarative suggestions per current Rust ecosystem convention ("a schema with a similar name exists: `X`"). The underlying fuzzy matching using `strsim` is sound; the framing should be updated.

**`--expand` output depth** is a progressive disclosure decision. Deeply recursive schema inlining can produce enormous output. Consider adding a `--depth` flag to cap recursion (e.g., `--depth 2` for two levels of inlining), with unlimited depth as the default when `--expand` is specified. This makes the flag usable without fear of overwhelming output.

**The `init` command** should be designed from the start to be fully non-interactive when flags are supplied. This ensures it works in dotfile automation and onboarding scripts.

**Color** should be used to mark structure, not to decorate. Resource names in bold or a consistent color, schema types in another, deprecated markers in yellow — all paired with text labels so the information is conveyed without color too.

**Shell completions** via `clap_complete` would significantly improve day-to-day ergonomics for a tool used repeatedly against different specs. Adding a `completions` subcommand is low-effort relative to the UX gain.

---

## Primary Sources

- [Command Line Interface Guidelines (clig.dev)](https://clig.dev/)
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/)
- [Command Line Applications in Rust (Official Book)](https://rust-cli.github.io/book/index.html)
- [Managing Colors in Rust](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html)
- [Rain's Rust CLI Recommendations – Handling Arguments](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html)
- [Rain's Rust CLI Recommendations – Configuration](https://rust-cli-recommendations.sunshowers.io/configuration.html)
- [Tips on Adding JSON Output to Your CLI App](https://blog.kellybrazil.com/2021/12/03/tips-on-adding-json-output-to-your-cli-app/)
- [Comparison of Rust CLI Prompts (cliclack, dialoguer, inquire, promptly)](https://fadeevab.com/comparison-of-rust-cli-prompts/)
- [10 Design Principles for Delightful CLIs (Atlassian)](https://www.atlassian.com/blog/it-teams/10-design-principles-for-delightful-clis)
- [Rust Compiler Diagnostics Guide](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
- [Evil Martians – CLI UX Progress Displays](https://evilmartians.com/chronicles/cli-ux-best-practices-3-patterns-for-improving-progress-displays)
- [Rust CLI Book – Exit Codes](https://rust-cli.github.io/book/in-depth/exit-code.html)
- [Standard Exit Status Codes in Linux (Baeldung)](https://www.baeldung.com/linux/status-codes)
- [human-panic on GitHub](https://github.com/rust-cli/human-panic)
- [strsim on crates.io](https://crates.io/crates/strsim)
- [miette on crates.io](https://crates.io/crates/miette)
- [clap_complete on crates.io](https://crates.io/crates/clap_complete)
- [owo-colors on crates.io](https://crates.io/crates/owo-colors)
- [indicatif on crates.io](https://crates.io/crates/indicatif)
- [dirs on crates.io](https://crates.io/crates/dirs)
- [XDG Base Directory and Rust](https://blog.liw.fi/posts/2021/02/14/xdg-base-dirs-rust/)
- [Ubuntu CLI Guidelines – Verbosity Levels](https://discourse.ubuntu.com/t/cli-verbosity-levels/26973)
- [Ubuntu CLI Guidelines – Interactive Prompts](https://discourse.ubuntu.com/t/interactive-prompts/18881)
