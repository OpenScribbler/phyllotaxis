# Kitchen-Sink Review Fixes - Design Document

**Goal:** Address all 10 issues found during post-implementation review of the kitchen-sink feature — 2 medium severity display fixes and 8 low severity polish/feature items.

**Decision Date:** 2026-02-23

---

## Problem Statement

After completing all 24 beads for the kitchen-sink-coverage-gaps feature, a full walkthrough of every major command against both petstore and kitchen-sink fixtures revealed 10 issues. None are regressions — they're polish items that affect display quality, consistency, and feature completeness. Two look broken to users (empty request body, misleading constraint formatting); the rest are cosmetic or feature gaps.

## Proposed Solution

A single polish pass touching the extraction (`commands/`) and rendering (`render/`) layers. No model restructuring needed. All changes are leaf-node modifications to existing functions.

### Issue Breakdown

#### Display Fixes

**#1 Empty request body for non-schema content types** (Medium)
- **Where:** `render/text.rs`, `render/json.rs`
- **Problem:** `POST /admin/bulk-import` (text/csv) shows "Request Body (text/csv):" followed by nothing
- **Fix:** When `body.fields.is_empty() && body.options.is_empty()`, display `"  Raw body (no schema)"` instead of empty space

**#2 exclusiveMinimum/Maximum constraint formatting** (Medium)
- **Where:** `commands/resources.rs` (constraint-building logic)
- **Problem:** `accuracy_m` shows `min:0 exclusiveMinimum` — reads like two unrelated constraints
- **Fix:** Combine into mathematical operators: `>0` for exclusive minimum, `<100` for exclusive maximum. Regular min/max stays as `min:0`, `max:100`. When exclusive bounds appear without a corresponding min/max value, use the exclusive value directly as the operand.

**#4 Trailing whitespace on empty header descriptions** (Low)
- **Where:** `render/text.rs`
- **Problem:** Response headers with no description produce trailing whitespace
- **Fix:** Trim or skip the description column when empty

#### Type/Extraction Improvements

**#3 Array item types don't propagate** (Low)
- **Where:** `commands/resources.rs` `format_type_display()`
- **Problem:** Array with inline items shows `array` instead of `binary[]` or `string[]`
- **Fix:** When type is `array` and `items` is an inline type (not `$ref`), produce `{item_type}[]`. The `$ref` case already works (e.g., `TreeNode[]`).

**#5 Links duplicated in JSON output** (Low)
- **Where:** `render/json.rs`
- **Problem:** Links appear at both top-level (aggregated) and per-response in JSON
- **Fix:** Remove top-level `links` field from `EndpointDetailJson`. Links stay on individual responses where they're defined. Text output keeps top-level aggregation for progressive disclosure hints.

#### Feature Additions

**#6 Callback operation count in list** (Low)
- **Where:** `render/text.rs`
- **Problem:** Callback list doesn't show how many operations each callback has
- **Fix:** Append `(N operation(s))` after callback name in list view

**#7 --expand flag on endpoint view** (Low)
- **Where:** `main.rs`
- **Problem:** `get_endpoint_detail()` accepts `expand` param but CLI doesn't expose `--expand` on resources subcommand
- **Fix:** Add `--expand` flag to resources subcommand CLI parsing. Wire through to `get_endpoint_detail()`.

**#8 Search covers callbacks** (Low)
- **Where:** `commands/search.rs`
- **Problem:** Searching for "onEvent" or "callback" returns no results
- **Fix:** Add callback name/path matching to search module. Return matches in a new `callbacks` section.

**#9 Fuzzy matching for callback names** (Low)
- **Where:** `commands/callbacks.rs`
- **Problem:** `phyllotaxis callbacks onEven` (typo) gives "not found" with no suggestions
- **Fix:** Add `suggest_similar_callbacks()` using existing Jaro-Winkler pattern from `resources.rs`.

**#10 Callback count in overview** (Low)
- **Where:** `commands/overview.rs`, `render/text.rs`, `render/json.rs`
- **Problem:** Overview shows resource and schema counts but not callback count
- **Fix:** Pass callback count through `OverviewData`. Display in both text and JSON overview.

## Architecture

No new models or structs. All changes touch existing code:

```
src/commands/resources.rs   — #2 (constraint formatting), #3 (array item types)
src/commands/search.rs      — #8 (callback search)
src/commands/callbacks.rs   — #9 (fuzzy matching)
src/commands/overview.rs    — #10 (callback count)
src/main.rs                 — #7 (--expand flag wiring)
src/render/text.rs          — #1 (raw body), #4 (trailing whitespace), #6 (operation count)
src/render/json.rs          — #1 (raw body), #5 (remove top-level links)
tests/integration_tests.rs  — All issues get at least one test
```

## Key Decisions

| Decision | Choice | Reasoning |
|----------|--------|-----------|
| Scope | All 10 issues | Well-defined, low-risk, codebase is fresh in context |
| Empty body display | Show "Raw body (no schema)" | Communicates that a body is expected, just unstructured |
| Exclusive bounds format | Operators: `>0`, `<100` | Concise, unambiguous, works for humans and LLMs alike |
| JSON links | Remove top-level aggregation | JSON consumers get links where defined (per-response); text keeps aggregation |
| Model changes | None needed | All fixes in commands/render layers only |

## Data Flow

No changes to the parsing or model pipeline. Fixes are applied at two levels:

1. **Extraction level** (commands/): Constraint formatting (#2), array type propagation (#3), callback search (#8), fuzzy matching (#9), overview data (#10)
2. **Rendering level** (render/): Display formatting (#1, #4, #5, #6), CLI wiring (#7)

## Error Handling

No new error paths. Existing error handling covers all cases:
- Fuzzy matching (#9) follows the same pattern as existing `suggest_similar()` functions
- Search (#8) returns empty results when no callbacks match
- Empty body (#1) is a display-only change

## Testing Strategy

Each fix gets an integration test against the kitchen-sink fixture. The fixture already contains the necessary data:
- `POST /admin/bulk-import` (text/csv) → #1
- `GeoLocation.accuracy_m` (exclusiveMinimum) → #2
- `POST /files/upload-batch` files field (binary array) → #3
- `HEAD /health` X-Health-Status header → #4
- `POST /users` links → #5
- Callbacks on `/notifications/subscribe` → #6, #8, #9, #10
- `POST /pets` with Owner ref → #7

## Success Criteria

- All 10 issues resolved with passing integration tests
- `cargo test` passes with no regressions on petstore tests
- Manual walkthrough of kitchen-sink fixture shows clean output for all affected commands

## Open Questions

None — all design decisions resolved during brainstorm.

---

## Next Steps

Ready for implementation planning with the Plan skill.
