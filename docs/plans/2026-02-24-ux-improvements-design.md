# UX Improvements - Design Document

**Goal:** Fix 8 UX friction points discovered during hands-on LLM testing of the phyllotaxis CLI against the kitchen-sink fixture.

**Decision Date:** 2026-02-24

---

## Problem Statement

Phyllotaxis targets LLMs as first-class users, but hands-on testing revealed friction points that cost extra round-trips and waste tokens. The two highest-impact issues: search can't find schema field names (searching "email" returns nothing despite 4+ schemas having email fields), and `--expand` doesn't inline array-of-`$ref` fields (so `ErrorDetail[]` doesn't expand, defeating the purpose of `--expand`). Six additional polish items affect output clarity and navigation consistency.

## Proposed Solution

Eight targeted changes across existing source files. No new commands, no new flags, no architectural changes. Each fix is isolated to 1-2 files with additive-only data model changes (new `Option` fields on existing structs).

## The Eight Changes

### 1. Search indexes schema field names (High)
**Files:** `search.rs`
**What:** When searching schemas, also match against field names within each schema (not just the schema name). When a field matches, annotate the result: `User (field: email)`.
**Model change:** `SchemaMatch` gets `matched_field: Option<String>`.
**How:** Search iterates `components.schemas`, resolves each to its properties, checks field names against the term.

### 2. `--expand` inlines array-of-ref fields (High)
**Files:** `schemas.rs`
**What:** The expand logic already handles object `$ref` fields but skips array items with `$ref`. Fix: when building a field from an array schema whose `items` is a `$ref`, and expand is active, resolve and recurse into it (same as object path). The depth cap (5) prevents infinite recursion.
**Model change:** None — `nested_fields` already exists on `Field`.

### 3. Search results show match reason (Medium)
**Files:** `search.rs`, `text.rs`, `json.rs`
**What:** Add `matched_on: Option<String>` to `EndpointMatch`. When a match comes from a parameter name/description rather than path/summary/description, populate it with e.g. `"parameter: session_token"`. Render as parenthetical in text, direct field in JSON.

### 4. Suppress empty parameter sections (Medium)
**Files:** `text.rs`
**What:** Stop rendering "Path Parameters: (none)" and "Query Parameters: (none)" in endpoint detail text output. Only render parameter sections that have entries. JSON output unchanged (empty arrays are fine for machine parsing).

### 5. Consistent drill-deeper hints (Low)
**Files:** `text.rs`
**What:** Add "Drill deeper:" hints to views that lack them:
- Schema listing → `phyllotaxis schemas <name>`
- Auth view → `phyllotaxis resources <name>` to see endpoints using a scheme

### 6. Search result counts (Low)
**Files:** `text.rs`, `json.rs`
**What:** Add a summary line at the top of search results: `Found 9 endpoints, 5 schemas matching "user"`. In JSON, add top-level count fields.

### 7. NonAdminRole type display (Low)
**Files:** `schemas.rs`, `text.rs`
**What:** When a schema has no fields and no composition, extract the underlying type from the OpenAPI schema (string, integer, etc.) and display it. Output: `Schema: NonAdminRole (string)` with description. Handles simple type alias schemas.
**Model change:** `SchemaModel` gets `base_type: Option<String>`.

### 8. Field alignment fix (Low)
**Files:** `text.rs`
**What:** Normalize column alignment in endpoint detail request body output so constraint annotations (`min:8`) align consistently across rows.

## Architecture

No new modules or structs. All changes modify existing code:

**Data model changes (additive only):**
- `SchemaMatch` in `search.rs`: add `matched_field: Option<String>`
- `EndpointMatch` in `search.rs`: add `matched_on: Option<String>`
- `SchemaModel` in `models/schema.rs`: add `base_type: Option<String>`

**Search flow:**
```
search() currently:
  schemas → filter by name → SchemaMatch { name }

search() after:
  schemas → filter by name OR field name →
    SchemaMatch { name, matched_field: Some("email") | None }

  endpoints → existing match logic →
    EndpointMatch { ..., matched_on: Some("parameter: session_token") | None }
```

**Expand flow:**
```
build_field() currently:
  if schema is $ref object → resolve, populate nested_fields
  if schema is array with $ref items → set type to "RefName[]", NO nested_fields

build_field() after:
  if schema is array with $ref items AND expand=true →
    resolve the $ref, build nested_fields from its properties (same as object path)
```

**Rendering changes:**
All conditional rendering and format adjustments in `text.rs`. JSON renderers get new fields from updated structs via serde derive.

## Key Decisions

| Decision | Choice | Reasoning |
|----------|--------|-----------|
| Search field match display | Schema-level with "(field: X)" annotation | Keeps search granularity consistent; avoids complex endpoint cross-referencing |
| Expand array-of-ref rendering | Inline under field, indented | Matches existing object-ref expand pattern; no new visual paradigm |
| Empty params in JSON | Keep empty arrays | Machine consumers expect stable schema; only suppress in text |
| Scope | All 8 issues | Mix of high-impact and quick wins; all improve LLM UX |

## Error Handling

None of these changes introduce failure modes:
- Field search is best-effort: if a schema can't be resolved, skip it
- Expand already has depth capping at 5
- Match annotations are `Option` fields: `None` means "matched on name/path as before"
- All struct changes are additive (new optional fields)

## Testing Strategy

Each change maps to specific test cases against the kitchen-sink fixture:
1. Search "email" → should return User, CreateUserRequest, etc. with `matched_field`
2. `schemas Error --expand` → should inline ErrorDetail fields under `details`
3. Search "authentication" → EndpointMatch should include `matched_on: "parameter: session_token"`
4. Endpoint with no path params → text output should not contain "Path Parameters:"
5. Schema listing → should end with drill-deeper hint
6. Search "user" → should include summary counts
7. `schemas NonAdminRole` → should show base type
8. POST /users request body → constraints should align

Existing tests should not break — all changes are additive.

## Success Criteria

- Searching "email" returns schema matches with field annotations
- `--expand` on Error inlines ErrorDetail fields
- All 200+ existing tests still pass
- Text output is cleaner (no empty sections, consistent hints)
- JSON output gains new optional fields without breaking existing consumers

## Open Questions

None — all design decisions resolved during brainstorm.

---

## Next Steps

Ready for implementation planning with `Plan` skill.
