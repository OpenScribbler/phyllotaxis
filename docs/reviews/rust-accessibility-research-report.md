# Rust CLI Accessibility Research Report

**Subject:** Accessibility best practices for Rust CLI applications, with focus on phyllotaxis
**Date:** 2026-02-21
**Scope:** Standards, crate ecosystem, output design, screen reader compatibility, cognitive load

---

## 1. The Accessibility Landscape for CLI Tools

CLI tools occupy a peculiar space in the accessibility world. Unlike web applications, there is no ARIA, no accessibility tree, and no standardized semantic structure for assistive technologies to parse. A 2021 ACM CHI study on blind developers found that most assistive technologies struggled to interpret non-linear or visually formatted CLI output — participants frequently resorted to workarounds like redirecting output to files or relying on `--json` flags. The preference was consistently for "bare mode" output: free of decorative characters, color, and animation.

This matters directly for phyllotaxis. As a tool for developers who work with OpenAPI specs — a technical audience that may include screen reader users, colorblind developers, or users on minimal terminal environments (CI, remote SSH, `TERM=dumb`) — the bar for accessibility should be intentionally high from the start.

Sources: [Accessibility of Command Line Interfaces (ACM CHI 2021)](https://research.google/pubs/accessibility-of-command-line-interfaces/), [Accessible by Design (AFixt)](https://afixt.com/accessible-by-design-improving-command-line-interfaces-for-all-users/)

---

## 2. Color and Contrast

### The Core Rule

Never use color as the **sole** means of conveying information. The PatternFly CLI Handbook states this directly: "Don't convey meaning through color alone." A red error indicator that is not accompanied by the word "Error" fails users with color vision deficiency and fails users on terminals that do not support color.

Practical implications for phyllotaxis:
- Status indicators must pair visual color with text labels ("Error:", "Warning:", "Success:")
- Section separators or visual hierarchy should work without color (bold/underline are safer alternatives)
- If using color to distinguish resource types or schema properties, always include a non-color differentiator

### Color Overuse

The CLI Guidelines (clig.dev) put it well: "If everything is a different color, then the color means nothing and only makes it harder to read." Color is most useful when it is exceptional — highlighting a critical value, marking an error, or drawing attention to a single key piece of output. When every line is colored differently, the signal is lost and the cognitive load increases for all users.

Sources: [PatternFly CLI Handbook](https://www.patternfly.org/developer-resources/cli-handbook/), [Command Line Interface Guidelines](https://clig.dev/)

---

## 3. The NO_COLOR and CLICOLOR Standards

### NO_COLOR (no-color.org)

The [NO_COLOR standard](https://no-color.org/) is the most widely adopted color suppression mechanism. The spec is:

> Command-line software which adds ANSI color to its output by default should check for a `NO_COLOR` environment variable that, when present and not an empty string (regardless of its value), prevents the addition of ANSI color.

Key implementation points:
- Check for the variable's **presence**, not its value (`NO_COLOR=0` still disables color)
- The empty string is the one exception — `NO_COLOR=""` does NOT trigger suppression
- Command-line flags (e.g., `--color=always`) may override `NO_COLOR` — this is explicitly permitted
- Over 650 libraries and applications now support this standard

### CLICOLOR / CLICOLOR_FORCE

The [CLICOLOR standard (bixense.com/clicolors)](http://bixense.com/clicolors/) adds a complementary set of variables:
- `CLICOLOR=0`: Equivalent to `--color=never` (disable color when not piped)
- `CLICOLOR_FORCE=1`: Equivalent to `--color=always` (force color even in pipes/CI)

Priority ordering (as implemented by compliant crates): `NO_COLOR` > `CLICOLOR_FORCE` > `CLICOLOR` > TTY detection

### TERM=dumb

When `TERM=dumb`, the terminal cannot handle ANSI escape codes. Any well-behaved CLI should detect this and disable all styling. The clig.dev guidelines explicitly list `TERM=dumb` as a condition requiring color suppression.

---

## 4. Rust Crate Ecosystem for Color and Terminal Detection

This is the most practically important section for phyllotaxis, given it already uses `clap`.

### The anstyle/anstream Ecosystem (Recommended)

The `clap` crate (v4+) is built on the `anstyle` ecosystem. This is the natural fit for phyllotaxis:

- **[`anstyle`](https://github.com/rust-cli/anstyle)**: Core style/color definitions (ANSI). The data type for expressing a style.
- **[`anstream`](https://epage.github.io/blog/2023/03/anstream-simplifying-terminal-styling/)**: Stream wrapper with automatic capability detection. Strips ANSI codes when `NO_COLOR` is set, `CLICOLOR=0` is set, output is piped to a file, or `TERM=dumb`. Uses `std::io::Write` as its API, making rendering code output-destination-agnostic.
- **[`colorchoice`](https://crates.io/crates/colorchoice)**: Global color choice state (Always/Auto/Never).
- **[`colorchoice-clap`](https://crates.io/crates/colorchoice-clap)**: Integrates color choice directly with clap argument parsing. Provides a `--color` flag automatically.

Since phyllotaxis already depends on `clap`, these crates are either already transitive dependencies or trivially addable. `colorchoice-clap` is the lowest-effort path to full `NO_COLOR` and `CLICOLOR` compliance with a user-facing `--color` flag.

### Clap's Built-in ColorChoice

Clap exposes a `ColorChoice` enum (`Auto` / `Always` / `Never`). The default is `Auto`, which performs TTY detection. However, clap's built-in `Auto` does **not** natively handle the `NO_COLOR` environment variable — you need `colorchoice-clap` or `anstream` for that.

### TTY Detection

As of Rust 1.70, the standard library provides `std::io::IsTerminal` — the recommended way to detect whether stdout/stderr is connected to a real terminal. Use this instead of the deprecated `atty` crate:

```rust
use std::io::IsTerminal;

if std::io::stdout().is_terminal() {
    // enable color, animations, etc.
}
```

When output is piped (e.g., `phyllotaxis resources | jq .`), `is_terminal()` returns false and all formatting should degrade to plain text.

### Other Notable Crates

| Crate | Purpose | NO_COLOR | CLICOLOR |
|-------|---------|----------|----------|
| [`colored`](https://crates.io/crates/colored) | Simple colorized output | Yes | Yes |
| [`owo-colors`](https://crates.io/crates/owo-colors) | Zero-alloc coloring, stylesheet pattern | Via `supports-color` | Via feature |
| [`supports-color`](https://crates.io/crates/supports-color) | Detection only, no styling | Yes | Yes |
| [`termcolor`](https://crates.io/crates/termcolor) | Cross-platform (legacy Windows API) | No | No |

The `termcolor` crate is widely mentioned but generally discouraged for new code — it targets deprecated Windows Console APIs and has a complex API. Prefer `anstream` + `anstyle`.

Sources: [Managing colors in Rust (Rain's recommendations)](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html), [anstream blog post](https://epage.github.io/blog/2023/03/anstream-simplifying-terminal-styling/), [colorchoice-clap](https://crates.io/crates/colorchoice-clap)

---

## 5. Screen Reader Compatibility

### How Terminal Screen Readers Work

Screen readers for terminals operate differently from web screen readers. They read text as it appears on screen, character by character or line by line, without any semantic structure to navigate. There is no equivalent of HTML headings or ARIA landmarks.

Platform breakdown:
- **Windows**: NVDA (free, open source, catches ~90% of issues) and JAWS (commercial). Both work with terminal emulators like Windows Terminal and cmd.exe.
- **Linux (graphical)**: Orca (GNOME). Compatible with GNOME Terminal but not xterm — two terminals that look visually identical can have completely different accessibility behavior.
- **Linux (CLI)**: Fenrir and Emacspeak are purpose-built for command-line environments. Speakup works in Linux console mode.

### What Breaks Screen Reader Compatibility

- **ANSI escape codes**: These appear as raw character sequences (e.g., `\e[31m`) rather than as invisible styling. When a CLI doesn't detect a non-TTY environment, these codes pollute the output and are read aloud as garbage.
- **Progress spinners and animations**: Animated output causes screen readers to constantly re-read the same line, creating noise. When stdout is not a TTY, animations must be disabled entirely.
- **ASCII art and box-drawing characters**: Box-drawing Unicode is often read aloud as character names ("box drawings light horizontal") rather than ignored. The same applies to decorative separators.
- **Non-linear output**: Tables that span columns may read left-to-right instead of row-by-row, losing the relationship between headers and values.
- **Dense symbol use**: Emoji and special Unicode symbols interrupt the reading flow and are read by their Unicode name, not by context.

### Design Patterns That Help

- **Linear output**: Each meaningful piece of information on its own line, labeled explicitly. `Key: value` format reads well. Tables do not.
- **Plain separators**: A blank line between sections is far more screen-reader-friendly than `━━━━━━━━━━`.
- **Labeled status**: "Error: authentication required" rather than a red symbol alone.
- **Suppress color when not TTY**: The most impactful single change.

Sources: [Accessibility testing using Linux](https://www.makethingsaccessible.com/guides/accessibility-testing-using-linux/), [Screen Reader Survey (Accessing Higher Ground)](https://accessinghigherground.org/survey-of-screen-readers-inlinux-operating-systems/)

---

## 6. Plain Text Fallbacks and Output Degradation

Well-designed CLI output degrades gracefully through a stack of environments:

1. **Interactive TTY** — Full color, Unicode symbols, optional progress indicators
2. **Non-TTY pipe** — Plain text, no color, no animation, structured and parseable
3. **TERM=dumb or NO_COLOR** — Same as pipe mode, regardless of TTY status
4. **`--json` flag** — Fully machine-readable structured output

The principle from clig.dev: "Humans come first, machines second" — but the two are not in conflict if designed together. The `--json` flag handles the machine-readable case explicitly; the plain text default should be clean enough to `grep` without issue.

For phyllotaxis specifically: the `resources`, `schemas`, `auth`, and `search` commands output structured data about OpenAPI specs. Each of these is a natural candidate for both a human-readable default and a `--json` alternative. The JSON output path also serves as an accessibility feature for users who need to pipe output into their own tools for processing.

---

## 7. Machine-Readable Output as an Accessibility Feature

JSON output is not just a developer convenience — it is an accessibility feature for users who cannot effectively parse visual terminal output. This includes:

- Screen reader users who redirect output to a file and process it externally
- Users who pipe into `jq` for filtering
- Automation and CI environments
- Users with cognitive or processing differences who benefit from structured, predictable data formats

Implementation best practices (from CLI community consensus and the clig.dev guidelines):
- JSON output goes to **stdout only** — errors remain on stderr in human-readable form
- Schema should be consistent — don't change field names between versions
- When `--json` is active, suppress all decorative output including headers and separators
- A clean `--json` output that can be piped to `jq .resources[0].path` without additional cleanup is the target

---

## 8. Unicode and Special Characters

### The Compatibility Problem

Unicode rendering in terminals depends on three things: terminal font support, system locale/encoding, and terminal application support. Failure at any layer produces replacement characters or question marks. Common failure scenarios:
- Windows terminals with non-UTF-8 encoding (CP437, CP850)
- Minimal Linux environments without font coverage for extended Unicode ranges
- SSH sessions where locale settings differ from the local machine

### Safe Unicode vs. Problematic Unicode

Characters from the Basic Multilingual Plane (BMP) have the widest font coverage. Box-drawing characters (U+2500 range), arrows (U+2190–U+21FF), and common symbols (✓, ✗, ●, →) are broadly supported. Characters from supplementary planes — emoji, many "Miscellaneous Technical" symbols — have inconsistent support and should be avoided in default output.

### Practical Strategy

1. **Detect Unicode support**: Check `LANG`, `LC_ALL`, and `LC_CTYPE` for `UTF` — if absent, assume ASCII-only.
2. **Provide ASCII fallbacks**: Map symbols to ASCII equivalents:

| Unicode | Meaning | ASCII Fallback |
|---------|---------|----------------|
| ✓ (U+2713) | success/check | `[OK]` or `ok` |
| ✗ (U+2717) | failure/cross | `[!]` or `err` |
| → (U+2192) | pointer/next | `->` or `>` |
| ⚠ (U+26A0) | warning | `[!]` or `warn` |
| ● (U+25CF) | bullet | `*` |
| … (U+2026) | ellipsis | `...` |

3. **Respect TERM=dumb as an ASCII signal**: If `TERM=dumb`, avoid all Unicode beyond basic ASCII.
4. **Consider a `--no-unicode` flag**: For users who know their terminal can't render Unicode correctly, a flag provides an explicit escape hatch.

Sources: [cross-platform-terminal-characters (GitHub)](https://github.com/ehmicky/cross-platform-terminal-characters), [figures (sindresorhus)](https://github.com/sindresorhus/figures)

---

## 9. Verbosity and Information Density

### The Problem with Dense Output

Walls of output with no clear hierarchy are cognitively expensive for all users — but are particularly hard for screen reader users, who cannot skim. The 2021 ACM CHI study found that screen reader users frequently struggled with unstructured text output precisely because there is no visual skim available to them.

### Verbosity Tiers

A standard four-tier model from the [Ubuntu CLI Verbosity Levels](https://discourse.ubuntu.com/t/cli-verbosity-levels/26973) specification:

| Mode | Flag | What is shown |
|------|------|---------------|
| Quiet | `--quiet` / `-q` | Errors only |
| Brief (default) | *(none)* | Human-friendly result summaries |
| Verbose | `--verbose` / `-v` | Detailed operation descriptions |
| Debug | `--debug` | Internal execution steps |

For phyllotaxis, the most immediately useful distinction is quiet vs. default. A `-q` flag that suppresses all decorative output (headers, separators, success messages) while still printing the requested data is useful both for scripting and for accessibility.

### Information Architecture

From clig.dev: "Increase information density — but with structure." The goal is not minimal output, but appropriately structured output. The guidance:
- Group related items; blank lines between logical sections help screen readers navigate
- Put the most important information last — users see the bottom of output after scrolling
- Labels before values: `Endpoint: /api/v1/users` not `/api/v1/users` on a line alone

---

## 10. Help Text Accessibility (`--help`)

Clap's `--help` output is colored by default but automatically strips colors when output is piped or `NO_COLOR` is set — this is handled by the `anstream` dependency. The help text itself should follow these principles:

- **Descriptive argument names**: `--output-format` is clearer than `-f`; short flags are convenient but long flags are necessary for discoverability
- **Concrete examples**: A section of usage examples in `--help` reduces the cognitive load of figuring out syntax from abstract descriptions
- **Consistent language**: If three commands all take an `--output` flag, use the same name. Don't use `--output` in one place and `--format` in another for the same concept
- **Avoid abbreviations in help text**: `auth` as a command name is fine; "authn" in help descriptions is not — spell out "authentication"
- **Progressive disclosure**: `-h` shows short help; `--help` shows full documentation. This is already clap's default behavior

A clig.dev insight worth implementing: put examples before exhaustive flag documentation. A user trying to understand `phyllotaxis search` benefits more from seeing `phyllotaxis search "User"` than from reading a complete flag table.

---

## 11. Cognitive Load and Command Design

Cognitive load theory distinguishes between intrinsic complexity (the task itself) and extraneous complexity (bad design). CLI design can't change the intrinsic complexity of navigating an OpenAPI spec, but it can eliminate extraneous friction.

### Command Naming

- Use consistent noun-verb or verb-noun patterns across all commands. The `docker container create` / `kubectl get pods` models both work; mixing them in the same tool is confusing.
- Avoid near-synonyms: don't have a command called `list` and another called `get` that do similar things.
- Short names should be obvious: `auth` for authentication is conventional; `res` for resources is ambiguous.

### Predictability

- If `--json` works for one command, it should work for all commands that produce output
- If one command respects `--quiet`, all commands should
- Flag names should not change between subcommands (`--endpoint` vs `--url` for the same concept on different subcommands creates confusion)

### Error Messages

Poor error messages are both an accessibility failure and a cognitive load problem. The ACM CHI study found that cryptic error output was a top frustration for screen reader users. Best practice:
- State what went wrong in plain English
- State what to do next, if known
- Avoid showing raw stack traces or internal error codes by default (make them available via `--debug` or `--verbose`)
- Error messages go to stderr, not stdout, so they don't pollute pipes

---

## 12. Practical Checklist for phyllotaxis

Based on this research, the following are the highest-impact accessibility improvements for a Rust CLI tool like phyllotaxis:

### Must-Have (Blocking Issues)

- [ ] Strip ANSI color codes when stdout/stderr is not a TTY (`std::io::IsTerminal`)
- [ ] Respect `NO_COLOR` environment variable (present + non-empty = no color)
- [ ] Respect `TERM=dumb` (disable all styling)
- [ ] Ensure `--json` output contains no ANSI codes or decorative characters
- [ ] Errors go to stderr; data goes to stdout

### Should-Have (Significant Accessibility Value)

- [ ] Support `CLICOLOR=0` / `CLICOLOR_FORCE=1` (use `colorchoice-clap` or `anstream`)
- [ ] Never use color as the sole conveyor of meaning — pair with text labels
- [ ] Provide ASCII fallbacks for Unicode symbols when `TERM=dumb` or locale is non-UTF-8
- [ ] Add `-q` / `--quiet` flag to suppress decorative output
- [ ] Consistent flag names across all subcommands (`--json`, `--quiet`, etc.)

### Nice-to-Have (Polish and Edge Cases)

- [ ] Usage examples in `--help` output, before the flag table
- [ ] A `--no-color` flag as an explicit user opt-out (in addition to `NO_COLOR` env var)
- [ ] Document accessibility features (color flags, `NO_COLOR` support) in README
- [ ] Test output with ANSI stripped — does it still communicate everything it needs to?

---

## 13. Key Standards and References

| Resource | URL | Relevance |
|----------|-----|-----------|
| NO_COLOR Standard | https://no-color.org/ | Core color suppression standard |
| CLICOLOR Standard | http://bixense.com/clicolors/ | Color enabling/forcing standard |
| CLI Guidelines (clig.dev) | https://clig.dev/ | Comprehensive CLI design guide |
| PatternFly CLI Handbook | https://www.patternfly.org/developer-resources/cli-handbook/ | Accessibility-focused CLI design |
| ACM CHI: Accessibility of CLIs | https://dl.acm.org/doi/fullHtml/10.1145/3411764.3445544 | Research on blind developer CLI usage |
| Rain's Rust CLI Recommendations | https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html | Rust-specific color management |
| anstream blog post | https://epage.github.io/blog/2023/03/anstream-simplifying-terminal-styling/ | anstyle/anstream ecosystem explanation |
| Ubuntu CLI Verbosity Spec | https://discourse.ubuntu.com/t/cli-verbosity-levels/26973 | Verbosity level conventions |
| colorchoice-clap (crates.io) | https://crates.io/crates/colorchoice-clap | clap integration for color env vars |
| std::io::IsTerminal | https://doc.rust-lang.org/beta/std/io/trait.IsTerminal.html | Standard library TTY detection |

---

*Report produced 2026-02-21 via web research (no-color.org, clig.dev, ACM CHI 2021 study, PatternFly, Rust crate documentation, Ubuntu CLI specifications, and supporting sources).*
