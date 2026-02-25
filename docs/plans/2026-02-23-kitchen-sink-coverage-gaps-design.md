# Kitchen Sink Coverage Gaps - Design Document

**Goal:** Address all 8 gaps identified from running phyllotaxis against the comprehensive kitchen-sink.yaml test fixture, making phyllotaxis handle the full breadth of OpenAPI 3.0 features.

**Decision Date:** 2026-02-23

---

## Problem Statement

After creating a kitchen-sink OpenAPI 3.0 test spec (tests/fixtures/kitchen-sink.yaml) that exercises every OAS feature, we identified 8 gaps in phyllotaxis's parsing and display capabilities. The most severe: multipart/form-data request bodies are completely invisible, making file upload endpoints appear empty. Several other features (response headers, callbacks, links, schema constraints) are silently dropped.

These gaps mean phyllotaxis can't provide complete progressive disclosure for APIs that use these common OAS features.

## Proposed Solution

Three categories of changes, ordered by complexity:

### Category A: Model Enrichment (7 items)
Extend existing extraction functions and model structs to capture data the `openapiv3` crate already parses but we currently ignore.

### Category B: Renderer Improvements
Display the newly captured data in text and JSON output formats.

### Category C: New Feature — Callbacks & Links Navigation
Add callback and link display to endpoint detail views, plus a new `phyllotaxis callbacks` subcommand for dedicated navigation.

## Architecture

### Gap #1: Non-JSON Request Bodies

**Current:** `extract_request_body` in resources.rs only checks `application/json`.

**Fix:**
- Iterate over all content types in the request body
- Extract fields from any content type that has a schema (multipart, form-urlencoded, JSON all use the same Schema Object structure)
- Store content type on `RequestBody` struct for display
- For multipart, extract encoding info (contentType overrides per field)
- Binary fields (`type: string, format: binary`) display as type "binary"

### Gap #2: Response Headers

**Current:** `Response` struct has no header storage.

**Fix:**
- Add `headers: Vec<ResponseHeader>` to Response model (name, type, description)
- Extract headers from each response in `get_endpoint_detail`
- Text renderer: "Headers:" section under responses that have them

### Gap #3: Callbacks and Links (New Feature)

**Links:**
- Display in endpoint detail after Responses section: link name, target operationId, parameter mappings, drill-deeper hint
- No new subcommand — links appear in context on the defining endpoint

Example output:
```
Links:
  GetCreatedUser -> getUser
    userId = $response.body#/id
    phyllotaxis resources users GET /users/{userId}
```

**Callbacks:**
- **Inline (on parent endpoint):** Summary showing callback name, HTTP method, URL expression, body schema
- **New subcommand:** `phyllotaxis callbacks` lists all callbacks; `phyllotaxis callbacks <name>` shows full detail including the callback's own request body, responses, and parameters

Example inline:
```
Callbacks:
  onEvent -> POST {callbackUrl}/events
    Body: EventPayload
  onStatusChange -> POST {callbackUrl}/status
    Body: inline object
```

Example subcommand:
```
$ phyllotaxis callbacks onEvent
Callback: onEvent
Defined on: POST /notifications/subscribe
URL: {$request.query.callbackUrl}/events

  POST
    Body: EventPayload
    Responses:
      200 Callback acknowledged
      410 Subscription cancelled by client
```

### Gap #4: Schema Constraints

**Current:** Field struct captures type, format, required, nullable, readOnly, default, example, enum, description. Missing all numeric/string/array/object constraints.

**Fix:**
- Add `constraints: Vec<String>` to Field — pre-formatted strings like `min:3`, `max:32`, `pattern:^[a-z]+$`
- Extract from SchemaData during `build_fields`
- Text renderer: append constraints inline after existing annotations

Format: `username  string  (required)  min:3 max:32 pattern:^[a-zA-Z0-9_-]+$`

**Design decision:** Inline display chosen because LLMs (primary audience) process tokens linearly and don't benefit from visual whitespace. Constraints are most useful in context.

### Gap #5: writeOnly

**Fix:** Add `write_only: bool` to Field, extract alongside readOnly, render as `[write-only]`.

### Gap #6: Deprecated Schema Properties

**Fix:** Add `deprecated: bool` to Field, extract from SchemaData, render as `[DEPRECATED]` tag.

### Gap #7: Schema Title

**Fix:** Add `title: Option<String>` to SchemaDetail, display as "Schema: GeoLocation (Geographic Location)" when title differs from schema name.

### Gap #8: Integer Enums

**Fix:** Current enum extraction handles string enums. Fix extraction to also handle integer/number enum values from `SchemaData.enumeration`.

## Key Decisions

| Decision | Choice | Reasoning |
|----------|--------|-----------|
| Request body parsing approach | Same extraction for all content types | OAS 3.0 uses identical Schema Object structure regardless of content type |
| Constraint display | Inline after type info | LLM consumers process linearly; compact > formatted |
| Links UX | Inline on endpoint detail | Links are contextual — seeing them in-place with the response they belong to |
| Callbacks UX | Both inline + subcommand | Inline for context, subcommand for callback-heavy specs |
| New fields on existing structs | All Optional/Vec (empty = hidden) | Graceful degradation — specs without these features render identically |

## Data Flow

```
OpenAPI YAML → openapiv3 crate → extraction (commands/*.rs) → model structs (models/*.rs) → renderers (render/*.rs)
```

All changes are in the middle three stages. The openapiv3 crate and CLI argument handling are unchanged.

## Error Handling

- **Missing data:** All new fields are `Option<T>` or `Vec<T>`. Renderers skip empty sections. No crashes.
- **Malformed data:** openapiv3 handles validation. If it parsed, we trust it.
- **Graceful degradation:** Unparseable callback URL expressions display as raw strings.

## Success Criteria

After all gaps are addressed, running phyllotaxis against kitchen-sink.yaml shows:

1. `POST /files/upload` — multipart fields visible (file: binary, description: string, tags: array)
2. `GET /users` — response headers shown (X-Total-Count, X-Rate-Limit-Remaining)
3. `POST /notifications/subscribe` — callbacks shown inline (onEvent, onStatusChange)
4. `phyllotaxis callbacks` — lists all callbacks; drill into onEvent shows full detail
5. `POST /users` — links shown (GetCreatedUser, ListUserPets) with parameter mappings
6. `User` schema — username shows constraints (min:3 max:32 pattern)
7. `CreateUserRequest` — password shows `[write-only]`
8. `PetBase` — legacy_code shows `[DEPRECATED]`
9. `GeoLocation` — renders as "Schema: GeoLocation (Geographic Location)"
10. `Priority` — shows enum values [0, 1, 2, 3, 4]
11. All existing tests pass (regression safety via kitchen-sink.yaml and petstore.yaml)

## Open Questions

None — all design decisions resolved during brainstorm.

---

## Next Steps

Ready for implementation planning with Plan skill.
