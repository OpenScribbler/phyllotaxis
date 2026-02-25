# Kitchen-Sink Review Findings

**Date:** 2026-02-23
**Context:** Post-implementation review after completing all 24 beads for the kitchen-sink-coverage-gaps feature. Walked through every major command against both petstore and kitchen-sink fixtures.

---

## What works well

- **Progressive disclosure pattern is solid.** The `resources -> resource detail -> endpoint detail` and `schemas -> schema detail` flow feels natural. Drill-deeper hints guide without clutter.
- **New features land cleanly.** Constraints, write-only/deprecated flags, response headers, links with drill commands, callbacks — they all show up in the right places without overwhelming the output.
- **Content type priority works correctly.** JSON-first, then multipart, then form-urlencoded, then whatever else. The `text/csv` bulk import correctly identifies the content type.
- **Search still works well** across the larger spec.
- **Callbacks subcommand** follows the same progressive disclosure pattern as resources and schemas — list then drill.
- **allOf field merging** on Dog schema correctly pulls in PetBase fields including the DEPRECATED `legacy_code`.
- **Discriminator display** on Pet oneOf is clear and actionable with drill commands.

---

## Issues found (ordered by impact)

### 1. Empty request body for non-schema content types

**Severity:** Medium — looks broken to users
**Where:** `POST /admin/bulk-import` (text/csv)

The output shows "Request Body (text/csv):" followed by literally nothing — an empty line then immediately "Responses:". This is technically correct (no schema to extract fields from for raw CSV), but it looks like a bug.

**Suggestion:** Either display "Raw body (no schema)" or omit the Request Body section entirely when there are no fields, no options, and no schema ref.

### 2. `exclusiveMinimum` constraint display is not useful

**Severity:** Medium — misleading output
**Where:** `schemas GeoLocation` — `accuracy_m` field shows `min:0 exclusiveMinimum`

The constraint `exclusiveMinimum` appears as a bare word with no value context. The user sees `min:0 exclusiveMinimum` which means "greater than 0 (exclusive)" but the formatting doesn't make that relationship clear. It reads like two unrelated constraints.

**Suggestion:** Combine them: `>0` or `min:0 (exclusive)`. Same treatment for `exclusiveMaximum`.

### 3. Array item types don't propagate to type display

**Severity:** Low — pre-existing limitation, not a regression
**Where:** `POST /files/upload-batch` — `files` field shows `array` instead of `binary[]`

When an array's items are inline (not a `$ref`), the items type doesn't appear. You get `array` instead of `string[]` or `binary[]`. The `$ref` case already works (e.g., `TreeNode[]`).

**Suggestion:** Extend `format_type_display` to look at `arr.items` for inline types and produce `{item_type}[]`.

### 4. Trailing whitespace on empty header descriptions

**Severity:** Low — cosmetic
**Where:** `HEAD /health` — `X-Health-Status  string  ` (trailing spaces)

When a response header has no description, the render outputs trailing whitespace.

**Suggestion:** Trim trailing whitespace or skip the description column when empty.

### 5. Links duplicated in JSON endpoint output

**Severity:** Low — API design question
**Where:** JSON output for `POST /users`

Links appear at both `responses[0].links` (correct — defined on that response) AND at the top-level `endpoint.links` (aggregated from all responses). This is by design for progressive disclosure, but JSON consumers may not know which to use.

**Suggestion:** Either document the pattern or remove the top-level aggregation from JSON output (keep it for text only).

### 6. Callbacks list doesn't show operation count

**Severity:** Low — nice to have
**Where:** `phyllotaxis callbacks`

Output shows callback name and where it's defined, but not how many operations each callback has. A callback with 5 operations looks the same as one with 1.

**Suggestion:** Add operation count: `onEvent (1 operation)  (on POST /notifications/subscribe)`

### 7. No `--expand` for endpoint request bodies

**Severity:** Low — feature gap
**Where:** `POST /pets` in petstore — `owner` field shows `Owner` type but can't be expanded

`phyllotaxis schemas Pet --expand` works to expand nested fields. But there's no equivalent for drilling into nested request body fields from the endpoint detail view. You have to separately run `phyllotaxis schemas Owner` to see what's inside.

**Suggestion:** Add `--expand` flag to the endpoint detail view that passes `expand: true` through to `extract_request_body`.

**Note:** Actually, `get_endpoint_detail` already accepts an `expand` parameter and passes it to `extract_request_body`. The issue is that the CLI doesn't expose the `--expand` flag on the `Resources` subcommand — it's only on `Schemas`. This would be a small CLI wiring change.

### 8. Search doesn't cover callbacks

**Severity:** Low — feature gap
**Where:** Searching for "onEvent" or "callback" returns no results

The search module only covers resources, endpoints, and schemas. Callbacks are invisible to search.

**Suggestion:** Add callback name/path matching to the search module, returning results in a new `callbacks` section.

### 9. No fuzzy matching for callback names

**Severity:** Low — consistency gap
**Where:** `phyllotaxis callbacks onEven` (typo) gives "not found" with no suggestions

Schemas and resources have Jaro-Winkler fuzzy matching for "did you mean?" suggestions. Callbacks don't.

**Suggestion:** Add `suggest_similar_callbacks()` with the same Jaro-Winkler pattern used for schemas and resources.

### 10. Overview doesn't show callback count

**Severity:** Low — cosmetic
**Where:** `phyllotaxis` (overview)

The commands section shows resource and schema counts but not callback count:
```
phyllotaxis resources    List all resource groups (6 available)
phyllotaxis schemas      List all data models (31 available)
phyllotaxis callbacks    List all webhook callbacks           <-- no count
```

**Suggestion:** Pass callback count through `OverviewData` and display: `phyllotaxis callbacks    List all webhook callbacks (2 available)`

---

## Summary

10 issues found. 2 medium severity (empty body display, exclusiveMinimum formatting), 8 low severity (cosmetic or feature gaps). No regressions found on petstore. The core functionality is solid — these are polish items for a future pass.
