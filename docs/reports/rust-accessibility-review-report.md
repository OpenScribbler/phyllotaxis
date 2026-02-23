# Accessibility Review Report — phyllotaxis

**Date:** 2026-02-21
**Reviewer:** Accessibility review of commit history HEAD (initial state, no prior commits)
**Scope:** Full codebase review against the rust-accessibility-research-report.md framework

---

## Summary

phyllotaxis has a solid structural foundation for accessibility: all output goes through a clean text/JSON renderer split, errors consistently go to stderr, and the text renderer avoids all decorative Unicode and ANSI color codes entirely. However, the application does not check any standard color environment variables (`NO_COLOR`, `CLICOLOR`, `TERM=dumb`), performs no TTY detection before rendering, and applies the same formatted output regardless of whether it is being piped or displayed interactively. Several medium-severity issues around columnar alignment, screen reader readability, and inconsistent help text also exist. No blocking rendering defects were found, but the absence of environment variable handling is a gap against current CLI standards.

---

## Findings

### Finding 1: No NO_COLOR, CLICOLOR, or TERM=dumb handling

**Severity:** High
**Location:** `src/main.rs` (entire file), `Cargo.toml` (dependencies)

**Issue:** The application checks none of the three standard color suppression environment variables. Neither `NO_COLOR`, `CLICOLOR=0`, nor `TERM=dumb` is read anywhere in the codebase. The Cargo.toml dependency list is: `clap`, `openapiv3`, `serde`, `serde_yaml`, `serde_json`, `indexmap`. There is no `anstream`, `colorchoice-clap`, `colored`, `owo-colors`, or any other crate that provides environment-aware color management.

This is partially mitigated by the fact that `render/text.rs` currently produces zero ANSI escape codes — all output is plain text with no color applied at all. The risk is forward-looking: as soon as any color or styling is added (which is the natural next step for a CLI with a review/disclosure purpose), there will be no safety net to strip it for users who have `NO_COLOR` set.

**Why it matters:** `NO_COLOR` is the most widely adopted color suppression standard, supported by over 650 tools. Users who set `NO_COLOR` — including screen reader users, colorblind developers, and CI/CD pipelines — have a reasonable expectation that any compliant tool will honor it. `TERM=dumb` is set automatically in many SSH sessions, container environments, and CI runners. Failing to honor these variables is the single most common accessibility failure in CLI tools.

**Recommendations:**

1. **Add `colorchoice-clap` (recommended):** This crate integrates directly with clap and provides a `--color [always|auto|never]` flag automatically, plus respects `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and TTY detection. Since phyllotaxis already depends on clap 4, this is a near-zero-effort path. Add `colorchoice-clap` to Cargo.toml and call `ColorChoice::init_clap()` before parsing arguments. All output that eventually goes through `anstream`'s wrappers will then be correctly stripped.

2. **Add `anstream` and check manually:** For more control, add `anstream` and wrap `stdout`/`stderr` in `AutoStream`. `anstream` reads `NO_COLOR`, `CLICOLOR`, and `TERM=dumb` automatically and strips ANSI codes from the wrapped writer when any of these signals are active. This approach requires using `anstream`'s `AutoStream` instead of `println!` directly, but keeps the color decision centralized.

3. **Manual environment check (minimal, no new dependency):** At startup in `main()`, read `std::env::var_os("NO_COLOR")` and `std::env::var_os("TERM")` and set a global boolean that controls whether any future color is emitted. This works but is brittle — it requires manually threading the flag through the entire render layer and is easy to forget in future additions.

---

### Finding 2: No TTY detection — formatted output is piped verbatim

**Severity:** High
**Location:** `src/main.rs:84`, `src/main.rs:94`, `src/main.rs:105`, and every other `println!("{}", output)` call

**Issue:** phyllotaxis uses `println!` unconditionally for all output, with no check of whether stdout is connected to a terminal. When output is piped — `phyllotaxis resources | grep pets` or `phyllotaxis schemas | jq .` (with `--json`) — the human-readable text format with its columnar alignment and multi-line sections is emitted unchanged into the pipe.

For a tool that doesn't yet use ANSI codes, this is not yet catastrophic, but it sets up a structural pattern where formatters never receive the signal they need to degrade gracefully. The research report notes that `std::io::IsTerminal`, available since Rust 1.70, is the standard way to check this.

Specific examples of output that is poorly suited to pipe processing even without ANSI codes:
- `render/text.rs:596` — `render_resource_list` emits a `"Resources:\n"` header and a `"Drill deeper:\n"` footer. When piped to `grep`, these appear as noise.
- `render/text.rs:263-265` — `render_schema_list` ends with a multi-line "Drill deeper" hint that pollutes grep output.
- `render/text.rs:439` — `render_search` emits a human-readable results header that makes the output non-trivially parseable without `--json`.

**Why it matters:** Pipe users — including screen reader users who redirect to a file, developers grepping output, and CI scripts — receive human-formatted output when they would be better served by clean, label-prefixed, one-value-per-line output. The ACM CHI 2021 study found that screen reader users frequently redirect terminal output to files precisely to deal with this; if the file contains decorative structure, the workaround fails.

**Recommendations:**

1. **Use `std::io::IsTerminal` to detect the pipe case:** In `main()`, call `std::io::stdout().is_terminal()`. Pass this boolean into the render functions. When not a TTY, text renderers can suppress headers/footers and switch to a more parseable format (e.g., one `slug: description` pair per line rather than columnar alignment).

2. **When `--json` is not requested and output is not a TTY, apply a lean text format:** Suppress the "Drill deeper" section and use simple `key: value` pairs rather than space-padded columns. This serves grep users, file-redirection users, and screen reader users.

3. **Treat `--json` as the explicit machine-readable path:** For users who always pipe, document that `--json` is the reliable, stable output format for scripting. This makes the non-TTY text fallback a quality-of-life improvement rather than a critical contract.

---

### Finding 3: Columnar alignment using space-padding is screen reader unfriendly

**Severity:** Medium
**Location:** `src/render/text.rs:183-201` (`render_param_section`), `src/render/text.rs:204-248` (`render_fields_section`), `src/render/text.rs:238-248`, `src/render/text.rs:415-428` (`render_schema_fields`)

**Issue:** Parameters and fields are formatted using space-padding to align columns across rows. The format string in `render_fields_section` (line 239) is:

```rust
"  {:<nw$}  {:<tw$}  {:<20}  {}{}\n"
```

This right-pads field names to `max_name` characters, types to `max_type` characters, and flags to 20 characters. The result is a visually aligned table when rendered in a monospace terminal, but becomes a stream of whitespace-padded tokens when read linearly by a screen reader.

For example, a schema with fields `id` and `description_text` would produce:

```
  id                string/uuid  (required, read-only)   Unique identifier
  description_text  string       (optional)              A longer description
```

A screen reader reads this left-to-right as: "space space i d space space space space space space space space space space space space string slash uuid..." — the relationship between field name and type is not labeled, just positional.

Similarly in `render_param_section` (line 192-200):

```rust
"  {:<width$}  {}  ({})  {}{}\n"
```

Parameters are aligned by name length but have no explicit `name:`, `type:`, `required:` labels.

**Why it matters:** Screen readers and users who cannot visually skim depend on labeled, linear output. The ACM CHI 2021 study specifically called out unlabeled columnar tables as a top frustration for blind developer participants. A screen reader user cannot "skim a column" the way a sighted user can.

**Recommendations:**

1. **Add explicit labels in the non-TTY text path:** When stdout is not a TTY (see Finding 2), switch to labeled output:
   ```
   Field: id
     Type: string/uuid
     Flags: required, read-only
     Description: Unique identifier
   ```
   This is more verbose but fully linear and unambiguous.

2. **Keep the columnar format for TTY, apply labels for non-TTY:** This is the most compatible approach. Sighted users get the compact table; piped/screen-reader users get labeled output. The render functions would accept an `is_tty: bool` parameter.

3. **Adopt a consistent `key: value` format throughout for the TTY output too:** Sacrifice some visual compactness for universal readability. `name (type, required): description` on one line is both scannable and audible. This is a more opinionated change to the interactive output design.

---

### Finding 4: Unicode arrow `→` used as a semantic separator

**Severity:** Medium
**Location:** `src/render/text.rs:148` (response schema reference), `src/render/text.rs:325-328` (discriminator mapping)

**Issue:** The right-arrow Unicode character `→` (U+2192) is used to convey the relationship between a response status code and its schema, and between a discriminator value and its target schema:

Line 148:
```rust
.map(|s| format!(" → {}", s))
```
Produces: `  201 Created → Pet`

Lines 325-328:
```rust
out.push_str(&format!(
    "    {:<width$}  → phyllotaxis schemas {}\n",
```
Produces: `    pet      → phyllotaxis schemas Pet`

The arrow is not decorative — it carries the semantic meaning "maps to" or "returns." A screen reader will read this as "right-pointing arrow" or "right arrow" (depending on the screen reader), which breaks the sentence flow: "two oh one Created right arrow Pet" rather than "201 Created returns Pet."

U+2192 is generally well-supported in modern fonts, but in environments where the locale is not UTF-8 (CI, SSH, containers), it may render as a replacement character `?` or be omitted entirely, dropping the relationship entirely.

**Why it matters:** When the `→` character is the only visual representation of the "maps to" relationship and it is read aloud as "right arrow" by a screen reader, or replaced with `?` in a non-UTF-8 terminal, the output loses meaning. The research report's Unicode table explicitly lists `→` and its ASCII fallback `->`.

**Recommendations:**

1. **Replace `→` with labeled text:** Change `→ Pet` to `schema: Pet` or `returns: Pet`. This is unambiguous in all environments and reads cleanly in all contexts. For example:
   - `  201 Created (schema: Pet)` instead of `  201 Created → Pet`
   - `    pet  -> phyllotaxis schemas Pet` using the ASCII fallback `->` for the discriminator

2. **Check `TERM=dumb` or locale and fall back to `"->"` there:** Add a utility function that returns either `→` or `->` based on whether unicode output is appropriate. This preserves the visual preference in capable terminals while degrading gracefully.

3. **Use a text label in the discriminator mapping as well:** The discriminator block currently reads as a two-column table with `→` as the separator. Switching to `value -> schema_name` (ASCII) is sufficient and universally readable.

---

### Finding 5: The `init` command mixes stderr and eprint for interactive prompts

**Severity:** Medium
**Location:** `src/commands/init.rs:124-143`, `src/commands/init.rs:176-201`

**Issue:** The `init` command uses `eprintln!` for informational messages and `eprint!` (without newline) for interactive prompts that expect user input on the same line. Examples:

Line 132:
```rust
eprint!("Enter the path to your OpenAPI spec file: ");
```

Line 142:
```rust
eprint!("Select a spec file (enter number) or type a path: ");
```

Line 177:
```rust
eprint!("Add another spec? Enter a name for the new spec (or press Enter to cancel): ");
```

There are two accessibility problems here:

First, all `init` output — both informational messages and interactive prompts — goes to stderr. This is technically correct for interactive prompts (it prevents the prompt from being captured in a pipe), but screen readers may not read stderr output from some terminal configurations where they read stdout.

Second, `eprint!` without `\n` means the prompt sits on the same line as the cursor, which is conventional for interactive prompts but problematic for screen readers that read line-by-line — the prompt may not be announced until a newline is emitted.

**Why it matters:** Users who rely on screen readers to navigate terminal output may not hear prompts that lack a trailing newline, particularly on terminals where the screen reader reads line-by-line rather than character-by-character. This can cause the `init` command to appear to hang with no audible indication of what is expected.

**Recommendations:**

1. **End each prompt with `\n` and read input separately:** Put the prompt on its own line and accept that the cursor drops to the next line. This is less visually conventional but is unambiguously audible:
   ```rust
   eprintln!("Enter the path to your OpenAPI spec file:");
   // cursor is on next line; user types and presses Enter
   ```

2. **Add a visual/text indicator before each prompt:** Prefix prompts with a clear label like `"Prompt: "` so they are identifiable in screen reader output if multiple messages appear in sequence.

3. **Document the `--spec` flag as the non-interactive alternative:** Since `init` is interactive by design, ensure the help text for `init` prominently states that `--spec <path>` can be used directly with any other command to bypass the interactive setup entirely. Screen reader users who find the interactive flow difficult should know the escape hatch.

---

### Finding 6: `render_resource_list` pads the alpha/deprecated marker with trailing spaces as alignment filler

**Severity:** Low
**Location:** `src/render/text.rs:582-598`

**Issue:** In `render_resource_list`, resources without a status marker use a hardcoded string of spaces as a placeholder to maintain column alignment:

```rust
let marker = if group.is_deprecated {
    "[DEPRECATED]"
} else if group.is_alpha {
    "[ALPHA]     "
} else {
    "            "  // 12 spaces
};
```

The `"            "` (12 spaces) is emitted for every non-marked resource, and `"[ALPHA]     "` (with trailing spaces) is emitted for alpha resources to pad to the same width as `[DEPRECATED]`. This is a visual alignment trick that works in a monospace terminal but produces spurious whitespace in screen reader output and when piped to text processing tools.

A screen reader reading the resource list would hear: "pets space space space space space space space space space space space space space space Pet management" for a non-marked resource, which is confusing noise.

**Why it matters:** Spurious whitespace from visual alignment leaks into all non-visual consumption paths. Screen readers may announce "blank" or simply pause for the run of spaces. Text processing tools like `awk` or `cut` will split on unexpected field boundaries.

**Recommendations:**

1. **Use conditional output rather than space padding:** Only emit the marker bracket when the marker is present. If column alignment is desired, use the `{:<12}` format specifier on the marker string and pass an empty string for unmarked groups, rather than a literal string of spaces:
   ```rust
   let marker = if group.is_deprecated { "[DEPRECATED]" }
                else if group.is_alpha { "[ALPHA]" }
                else { "" };
   // Then: format!("  {:<width$}  {:<12}  {}\n", ...)
   ```

2. **Drop the column alignment for markers entirely:** The marker `[DEPRECATED]` or `[ALPHA]` already stands out visually and semantically. It does not need to be right-padded to align with a column that most resources don't have. Removing the padding reduces noise without losing information.

---

### Finding 7: `--json` flag name is unconventional and help text lacks examples

**Severity:** Low
**Location:** `src/main.rs:17-18`, `src/main.rs:10`

**Issue:** The JSON output flag is named `--json` (defined at line 17-18):

```rust
/// Output in JSON format
#[arg(long, global = true)]
json: bool,
```

The flag name `--json` is short and functional, but the ecosystem convention for a flag that selects an output format is `--output-format json` or `--format json`. The `--json` boolean flag pattern is common enough to be acceptable, but it means the flag cannot grow to support other output modes (YAML, CSV, etc.) without a breaking change.

A larger concern is the absence of usage examples anywhere in the CLI help. The `#[command(about = "Progressive disclosure for OpenAPI specs")]` in `main.rs:10` is the only guidance a first-time user sees before reading subcommand help. The research report notes (citing clig.dev) that usage examples before the flag table significantly reduce cognitive load, and that screen reader users who cannot skim the help text benefit particularly from concrete examples.

**Why it matters:** A user trying `phyllotaxis --help` for the first time sees only abstract subcommand names without examples. A screen reader user has to listen to the full help output sequentially and has no ability to skim ahead to examples that would anchor their understanding.

**Recommendations:**

1. **Add `#[command(long_about = "...")]` with concrete examples to each subcommand:** Clap's `#[command(after_help = "...")]` attribute can add an "Examples:" section that appears after the flag table. For example:
   ```
   Examples:
     phyllotaxis resources               List all resource groups
     phyllotaxis resources pets          Show endpoints in the pets resource
     phyllotaxis resources pets GET /pets    Full endpoint detail
     phyllotaxis schemas Pet             View a specific schema
     phyllotaxis search "user"           Search across endpoints and schemas
   ```

2. **Keep `--json` as-is but document it consistently:** The flag already works globally and the name is clear enough. Adding examples that show `phyllotaxis resources --json | jq .resources[0].slug` in the help text would make the machine-readable path much more discoverable for automation users.

3. **Consider `--output=json` as a future-proof alternative:** If output format options expand, `--output=json` allows adding `--output=yaml` or `--output=csv` later without introducing a second flag that conflicts with `--json`. This is a design decision worth making deliberately before the API stabilizes.

---

### Finding 8: JSON output for `render_auth` and `render_search` uses model-internal field names that may be confusing

**Severity:** Low
**Location:** `src/render/json.rs:315-317` (`render_auth`), `src/render/json.rs:311-313` (`render_search`)

**Issue:** `render_auth` and `render_search` serialize their models directly using `serde_json::to_string_pretty`:

```rust
pub fn render_auth(model: &crate::commands::auth::AuthModel) -> String {
    serde_json::to_string_pretty(model).expect("serialize auth")
}

pub fn render_search(results: &crate::commands::search::SearchResults) -> String {
    serde_json::to_string_pretty(results).expect("serialize search results")
}
```

This means the JSON field names are whatever the Rust struct field names happen to be. For `AuthModel`, this produces `schemes`, `total_operations`, and within schemes: `scheme_type`, `usage_count`. For `SearchResults`, this produces `term`, `resources`, `endpoints`, `schemas`.

The field `scheme_type` (from `SecuritySchemeInfo`) is not renamed via `#[serde(rename = "...")]`, so it serializes as `scheme_type` with an underscore — inconsistent with the camelCase convention used in the schema detail JSON output, where fields like `nested_schema` and `read_only` appear (also with underscores, actually consistent). However, `EndpointSummaryJson` in `render_resource_detail` uses `deprecated` and `alpha` while the Rust model uses `is_deprecated` and `is_alpha` — these are renamed correctly in the explicit structs. The direct serialization path bypasses this curation.

More importantly, the JSON output for `render_auth` includes `total_operations` — an internal counting field that a user of the JSON output must understand to compute "what percentage of operations require this auth scheme." This is useful, but its name could be clearer (`operation_count` or `total_endpoint_count` would be more immediately understandable).

**Why it matters:** The research report notes that JSON output serves as an accessibility feature for users who cannot parse visual output. Inconsistent or opaque field names increase the cognitive overhead for these users, who are often already working with the output through an additional tool layer (jq, scripts, etc.).

**Recommendations:**

1. **Add `#[serde(rename_all = "camelCase")]` or audit field names on directly-serialized models:** Either pick snake_case consistently (which the codebase generally does) or add rename attributes. The current state is internally consistent but worth auditing as the JSON API stabilizes.

2. **Rename `total_operations` to a more descriptive field:** `total_endpoint_count` or `total_operations_in_spec` makes the field's scope clear without requiring the user to understand the internal counting logic.

3. **Write JSON output through explicit serialization structs (like other render functions do):** `render_overview`, `render_resource_list`, and `render_schema_detail` all define local `*Json` structs that allow deliberate control over field names and structure. `render_auth` and `render_search` skip this step. Aligning them with the pattern used elsewhere would give more control over the public JSON API surface.

---

### Finding 9: No `--quiet` / `-q` flag

**Severity:** Low (informational)
**Location:** `src/main.rs` (global flags, lines 9-26)

**Issue:** phyllotaxis has no `--quiet` or `-q` flag. Every command's text output includes both the data and surrounding context (headers, "Drill deeper" hints). For a user who just wants to extract a list of resource slugs, there is no way to get clean output without `--json` and post-processing with jq.

The research report references the Ubuntu CLI Verbosity Levels specification, which recommends a `--quiet` mode that suppresses decorative output while still printing the requested data.

**Why it matters:** Users who pipe output to scripts or screen readers that process output incrementally benefit from the ability to suppress the chrome (headers, footers, hints) while keeping the data. Currently the only options are full text (with decorative structure) or full JSON (requiring jq). A `--quiet` mode serves the gap between these.

**Recommendations:**

1. **Add a global `--quiet` / `-q` flag** that suppresses the "Drill deeper" section and section headers, outputting only the data rows. For `render_resource_list`, this would produce just one line per resource (`pets    Pet management`) without the `Resources:` header or the `Drill deeper:` footer.

2. **Combine with TTY detection (Finding 2):** When stdout is not a TTY, apply quiet behavior automatically. This is a reasonable default: a piped consumer rarely wants the decorative frame.

---

### Finding 10: No `atty` crate (positive finding) — but `std::io::IsTerminal` is not used either

**Severity:** Informational
**Location:** `Cargo.toml`

**Issue:** The research report warns against the `atty` crate, which was deprecated and found to be unsound (it had undefined behavior in a signal handler). phyllotaxis does not use `atty` — this is correct. However, it also does not use the safe replacement, `std::io::IsTerminal`, which has been stable since Rust 1.70. TTY detection is simply absent rather than implemented with the deprecated crate.

This is a positive finding in that no unsafe code is present, but the omission means none of the TTY-conditional behaviors described in other findings are possible without first adding this capability.

**Why it matters:** Informational — confirms no legacy unsound code, but reinforces that the entire TTY-detection layer needs to be built rather than replaced.

**Recommendations:**

1. **When implementing TTY detection (required for Findings 2, 3, 6), use `std::io::IsTerminal`** from the standard library:
   ```rust
   use std::io::IsTerminal;
   let is_tty = std::io::stdout().is_terminal();
   ```
   No crate dependency needed. This is the correct, future-proof approach per the research report.

---

## Prioritized Action List

The following are ordered by accessibility impact. The first three address the gap between current behavior and what users with accessibility needs (screen readers, `NO_COLOR`, non-UTF-8 environments) would expect from a compliant tool.

1. **Implement `NO_COLOR` and `TERM=dumb` environment variable checks** (Finding 1) — Add `colorchoice-clap` now, before any color is added to the renderers. This is the lowest-effort, highest-impact single change. It makes the app compliant with the most widely-adopted color standard in the CLI ecosystem and establishes the infrastructure for all future color additions.

2. **Add TTY detection using `std::io::IsTerminal`** (Finding 2 + Finding 10) — Detect whether stdout is connected to a terminal and pass the result into the render layer. This is a prerequisite for findings 3, 6, and 9. When not a TTY, suppress headers, footers, and alignment padding.

3. **Replace columnar alignment with labeled output in the non-TTY path** (Finding 3) — Once TTY detection is in place, switch `render_param_section`, `render_fields_section`, and `render_schema_fields` to a labeled `Key: value` format when piped. This directly improves screen reader and grep usability.

4. **Replace the `→` Unicode arrow with text labels** (Finding 4) — Change `" → Pet"` to `"(schema: Pet)"` or `"-> Pet"` in response rendering and discriminator mapping. This is a one-line change in `render/text.rs:148` and `render/text.rs:325-328` that eliminates a screen reader pronunciation problem and a non-UTF-8 rendering failure.

5. **Fix `render_resource_list` trailing-space padding** (Finding 6) — Replace the literal 12-space string placeholder with an empty string and use format-width specifiers to handle alignment. This removes spurious whitespace from screen reader output and piped text processing.

6. **Add interactive prompt accessibility improvements to `init`** (Finding 5) — End each `eprint!` prompt with `\n` and document `--spec` as the non-interactive bypass in help text.

7. **Add usage examples to `--help` output** (Finding 7) — Add `#[command(after_help = "...")]` sections with concrete examples to the top-level command and each subcommand. This is low-effort, high cognitive-load reduction.

8. **Add a `--quiet` / `-q` flag** (Finding 9) — Suppress headers and "Drill deeper" hints. Combine with TTY auto-quiet when stdout is not a terminal.

9. **Audit JSON field names for `render_auth` and `render_search`** (Finding 8) — Define explicit serialization structs (matching the pattern of other render functions) to give deliberate control over the public JSON API surface.
