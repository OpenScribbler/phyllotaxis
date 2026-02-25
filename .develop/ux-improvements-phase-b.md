# UX Improvements — Phase B Analysis
**Plan:** `docs/plans/2026-02-24-ux-improvements-implementation.md`
**Date:** 2026-02-24
**Verified against:** source files at HEAD (commit bb527a9)

---

## Summary

The plan is largely sound but contains six concrete issues ranging from a blocking
compile error to incorrect test assertions and one phantom type that doesn't exist. None
are showstoppers that require redesigning a task — all are fixable in a few lines. The
most serious is Issue T5-1: a unit test references `SchemaListModel`, a type that does
not exist anywhere in the codebase.

---

## Task 1: Search indexes schema field names

### Verification
- [x] `SchemaMatch { pub name: String }` exists at `src/commands/search.rs:35–37` — struct shape confirmed.
- [x] `find_schema()` exists at `src/commands/schemas.rs:20–52` with signature
  `fn find_schema<'a>(api: &'a openapiv3::OpenAPI, name: &str) -> Option<&'a openapiv3::Schema>`.
  The call in the Task 1 implementation snippet is compatible.
- [x] `spec::schema_name_from_ref()` exists at `src/spec.rs:244–251` with signature
  `pub fn schema_name_from_ref(reference: &str) -> Option<&str>`. Used in the AllOf
  property walk — mirrors the pattern at `schemas.rs:31`.
- [x] `SchemaKind::AllOf { all_of }` is a real variant in openapiv3 2.2.0.
- [x] Fixture: `User` schema at `kitchen-sink.yaml:920` has an `email` field.
  `CreateUserRequest` at line 984 has an `email` field. `PatchUserRequest` at line 1015
  has an `email` field. All three test assertions are correct.
- [x] Test `test_search_field_name_does_not_shadow_name_match` searches `"user"`. The
  name `"User"` contains `"user"` case-insensitively, so the name-match branch fires
  first and `matched_field` is `None`. Assertion is correct.
- [x] The existing `json.rs` test at line 455 constructs `SearchResults` with
  `schemas: vec![]` — no struct-init change needed for Task 1 alone. (Task 5 changes
  `SearchResults` itself, handled there.)

### Issues
- [x] No blockers. Line reference "~133–138" for the schema search block is accurate
  (`search.rs:133–138`). Verify before editing in case of prior edits.

---

## Task 2: `--expand` inlines array-of-ref fields

### Verification
- [x] `build_fields` is at `src/commands/resources.rs:113–229`. Signature:
  `pub fn build_fields(api: &openapiv3::OpenAPI, schema: &openapiv3::Schema, required_fields: &[String]) -> Vec<Field>`.
  The plan's implementation is compatible with this signature.
- [x] Root cause confirmed at `resources.rs:173–175`: `ReferenceOr::Item(boxed)` branch
  sets `schema_name = None`, so `nested_schema_name` is always `None` for inline schemas
  including array-of-ref fields. Plan's diagnosis is correct.
- [x] `ArrayType.items` is typed `Option<ReferenceOr<Box<Schema>>>` in openapiv3 2.2.0.
  The plan's pattern `Some(openapiv3::ReferenceOr::Reference { reference }) = &arr.items`
  binds `reference: &String`. The call `spec::schema_name_from_ref(reference.as_str())`
  is consistent with the existing usage at `resources.rs:298`. No type mismatch.
- [x] `ErrorDetail` at `kitchen-sink.yaml:1476–1484` has fields `field`, `reason`, and
  `value`. The test assertion checks for `"field"` or `"reason"` in the nested field
  names — correct.

### Issues
- [ ] **ISSUE T2-1 (note — `value` field type):** `ErrorDetail.value` has no `type:`
  property in the fixture, only a `description:`. This parses as
  `SchemaKind::Any(AnySchema { .. })`, and `format_type_display` returns `"object"` for
  it. The field appears in `nested_fields` with `type_display = "object"`. The test
  assertion only checks for the names `"field"` or `"reason"`, so this does not break
  the test. No action required — just be aware when inspecting expanded output manually.

---

## Task 3: Search results show match reason

### Verification
- [x] `EndpointMatch` at `search.rs:27–32` — confirmed no `matched_on` field yet.
- [x] `session_token` is a cookie parameter on `GET /users` at `kitchen-sink.yaml:107–110`.
  The parameter name is exactly `session_token`. Test assertion `Some("parameter: session_token")` is correct.
- [x] `GET /users` path is `/users`. Test assertion `e.path == "/users"` is correct.
- [x] `json.rs:render_search` at line 361 calls `serialize(results, is_tty)` directly on
  `SearchResults`, which derives `Serialize`. The new `matched_on` field on `EndpointMatch`
  propagates to JSON automatically via `#[serde(skip_serializing_if = "Option::is_none")]`.
  Plan's claim is correct.
- [x] Task 1 adds `matched_field` to `SchemaMatch`. Task 3 adds `matched_on` to
  `EndpointMatch`. These are independent struct changes on different structs — no conflict.

### Issues
- [ ] **ISSUE T3-1 (silent wrong behavior — variable shadowing, must fix):**
  The current code at `search.rs:86–92` uses a local variable named `desc_match` for the
  operation description check. The plan's new parameter loop also uses `desc_match`
  internally for parameter description matching (the `let desc_match = pdata.description...`
  line inside the loop). The plan's Implementation Notes section flags this and says to
  rename the operation-level variable to `op_desc_match` throughout the block. This rename
  **must be applied** — without it, the outer `desc_match` is shadowed inside the loop,
  making the `if path_match || summary_match || desc_match || param_match` condition check
  the wrong variable. The plan's note is correct; the rename is required.

---

## Task 4: Suppress empty param sections + field alignment fix

### Verification
- [x] `render_param_section` at `text.rs:263–294` unconditionally writes the section
  header then checks `if params.is_empty()`. Plan's diagnosis is correct.
- [x] `render_endpoint_detail` at `text.rs:129–133` calls `render_param_section`
  unconditionally for path and query params. Plan's diagnosis is correct.
- [x] `POST /users` at `kitchen-sink.yaml:140–186` has no `parameters:` key — confirmed
  no path or query parameters. The suppression tests are valid.
- [x] `GET /users/{userId}` at `kitchen-sink.yaml:188–196` declares `userId` as a
  required path parameter. `test_non_empty_path_params_still_shown` assertion is correct.
- [x] `CreateUserRequest` (`kitchen-sink.yaml:975–998`): `username` has `minLength: 3`
  → constraint `min:3`; `password` has `minLength: 8` → constraint `min:8`. Both appear
  in the POST /users request body. The column alignment test is valid.
- [x] `render_fields_section` at `text.rs:296–369` has current format string at line
  356–368 appending `constraints_str` after `desc` without separate alignment. Plan's
  diagnosis is correct.
- [x] `render_schema_fields` at `text.rs:507–580` has the same format string pattern
  at line 565–578. Plan correctly identifies both functions need the same fix.

### Issues
- [ ] **ISSUE T4-1 (test breakage risk — verify before committing):** After the fix,
  `render_param_section` is only called when params are non-empty, so any existing test
  that asserts `stdout.contains("Path Parameters:")` for an endpoint that has no path
  params will break. The plan's analysis concludes `test_resources_endpoint_get` is safe
  (it asserts `Query Parameters`, and `GET /pets` has a `limit` query param). This is
  correct, but the following verification command should be run immediately after
  implementing the suppression change to confirm no other tests break:
  ```
  cargo test -p phyllotaxis
  ```
  If any test fails on `"Path Parameters:"`, update the assertion to check for a specific
  param name instead.

- [ ] **ISSUE T4-2 (column order change — existing `contains` assertions still pass):**
  The plan moves constraints before description in the output. Any test that does
  `stdout.contains("some_description  min:3")` would break, but the plan correctly states
  no position-sensitive tests exist for this. Verify with `grep -rn "min:" tests/` before
  implementing to be sure.

---

## Task 5: Search result counts + consistent drill-deeper hints

### Verification
- [x] `SearchResults` at `search.rs:11–18` — confirmed no `endpoint_count` or
  `schema_count` fields yet.
- [x] `render_auth` at `text.rs:736–775` — current TTY drill-deeper block at lines
  769–772 is exactly as the plan shows. Plan target is accurate.
- [x] `render_schema_list` at `text.rs:371` **already accepts `is_tty: bool`**.
  The plan says "check whether it currently does" — it does. No signature change needed.
- [x] `render_schema_list` at `text.rs:383–386` **already has the TTY drill-deeper hint**:
  ```rust
  if is_tty {
      out.push_str("\nDrill deeper:\n");
      out.push_str("  phyllotaxis schemas <name>\n");
  }
  ```
  The existing test `test_render_schema_list` at `text.rs:1041–1053` already asserts
  `output.contains("Drill deeper:")` and `output.contains("phyllotaxis schemas <name>")`.

### Issues
- [ ] **ISSUE T5-1 (blocking compile error — phantom type):** Task 5's Step 1 unit test
  in the plan (lines 1082–1093) constructs:
  ```rust
  use crate::commands::schemas::SchemaListModel;
  let model = SchemaListModel { schemas: vec!["User".to_string(), "Error".to_string()] };
  let output = render_schema_list(&model, true);
  ```
  **`SchemaListModel` does not exist anywhere in the codebase.** `render_schema_list`
  takes `&[String]`, not a struct. This test will fail to compile. The correct form is:
  ```rust
  let names = vec!["User".to_string(), "Error".to_string()];
  let output = render_schema_list(&names, true);
  ```
  This is the same pattern as the existing `test_render_schema_list` at `text.rs:1042`.

- [ ] **ISSUE T5-2 (redundant — drill-deeper already implemented):** The schema list
  drill-deeper hint (plan Step 4) is already present in `text.rs:383–386`, and the
  existing unit test already covers it. The plan's `test_schema_listing_drill_deeper_hint`
  unit test is redundant. Adding a test with that exact name would be a duplicate of the
  existing `test_render_schema_list`. Skip Step 4 of Task 5 entirely — nothing to
  implement or test. The integration test `test_schema_listing_shows_drill_deeper_hint`
  (which checks that non-TTY output does NOT show the hint) is a valid new test and can
  be kept.

- [ ] **ISSUE T5-3 (build break — must update json.rs test in same commit):** Adding
  `endpoint_count: usize` and `schema_count: usize` to `SearchResults` is a struct change.
  The `json.rs:test_all_json_outputs_parse` at line 455 constructs `SearchResults`
  directly and will fail to compile until updated. The plan correctly identifies this
  (Step 2) and instructs adding the two fields with value `0`. This update must be in the
  same commit as the struct change, not deferred.

---

## Task 6: NonAdminRole base type display

### Verification
- [x] `NonAdminRole` at `kitchen-sink.yaml:1201–1208` uses `not:` — confirmed. Parses as
  `SchemaKind::Not { not: Box<ReferenceOr<Schema>> }`. Current `build_schema_model`
  catch-all `_ => (Vec::new(), None)` matches this, yielding empty fields and no
  composition. Plan's diagnosis is correct.
- [x] `SchemaModel` at `models/schema.rs:8–17` — confirmed no `base_type` field yet.
- [x] `SchemaKind::Not` in openapiv3 2.2.0 is a named-field variant:
  `Not { not: Box<ReferenceOr<Schema>> }`. The plan's wildcard pattern `Not { .. }` is
  valid Rust for named-field variants. No compile error.
- [x] `Type::Boolean` in openapiv3 2.2.0 is `Boolean(BooleanType)` — a tuple variant.
  The plan uses `openapiv3::SchemaKind::Type(openapiv3::Type::Boolean { .. })` with
  named-field syntax. **This is wrong syntax for a tuple variant.**
  See Issue T6-1 below.
- [x] `render_schema_detail` at `text.rs:391–505` — header block at lines 401–405 is
  exactly as the plan shows. Modification target is correct.

### Issues
- [ ] **ISSUE T6-1 (compile error — wrong Boolean pattern syntax):** Task 6 Step 3 uses:
  ```rust
  openapiv3::SchemaKind::Type(openapiv3::Type::Boolean { .. }) => Some("boolean".to_string()),
  ```
  `Type::Boolean` is a **tuple variant** `Boolean(BooleanType)`, not a struct variant.
  The `{ .. }` struct-wildcard syntax is invalid for a tuple variant. The correct pattern is:
  ```rust
  openapiv3::SchemaKind::Type(openapiv3::Type::Boolean(_)) => Some("boolean".to_string()),
  ```
  This matches the exact pattern already used in `resources.rs:294`:
  `openapiv3::Type::Boolean(_) => "boolean".to_string()`.
  Without this fix, the `base_type` derivation block will not compile.

- [ ] **ISSUE T6-2 (silent wrong output — `base_type` missing from JSON):** The plan
  states that `json.rs` picks up `base_type` "automatically via serde". This is incorrect.
  `json.rs:render_schema_detail` does not serialize `SchemaModel` directly — it builds a
  local `SchemaDetailJson<'a>` struct (lines 5–14) and manually maps each field from
  `SchemaModel`. `base_type` is not in `SchemaDetailJson` and is not copied in the
  constructor at lines 312–326. As a result, `"base_type"` will never appear in JSON
  output even after the model change. The verification command in the plan
  (`grep base_type`) would return nothing instead of `"base_type": "not"`.
  Fix: add `base_type` to `SchemaDetailJson` and populate it:
  ```rust
  // In SchemaDetailJson struct:
  #[serde(skip_serializing_if = "Option::is_none")]
  base_type: Option<&'a str>,

  // In the constructor (json.rs ~line 312):
  let json = SchemaDetailJson {
      // ... existing fields ...
      base_type: model.base_type.as_deref(),
  };
  ```

- [ ] **ISSUE T6-3 (build break — SchemaModel literal changes required):** Adding
  `pub base_type: Option<String>` to `SchemaModel` means all existing `SchemaModel { ... }`
  struct literals must include `base_type: None`. The plan correctly identifies this and
  lists the locations:
  - `src/render/json.rs:426–434` (first `SchemaModel` literal in `test_all_json_outputs_parse`)
  - `src/render/json.rs:489–497` (second `SchemaModel` literal)
  These two are the only struct literal constructions of `SchemaModel` in the codebase
  (all other usages go through `build_schema_model()` which returns `Option<SchemaModel>`).
  Both updates must be in the same commit as the struct field addition.

---

## Cross-task conflicts

- [ ] **TC-1 (Tasks 1+3+5 all touch `search.rs` structs):** Task 1 adds `matched_field`
  to `SchemaMatch`. Task 3 adds `matched_on` to `EndpointMatch`. Task 5 adds
  `endpoint_count`/`schema_count` to `SearchResults`. These are independent struct changes
  but all touch `search.rs` and the constructor at the bottom of `search()`. Execution
  order in the plan (1 then 3 then 5) is safe — each task's changes are additive and
  non-overlapping structurally.

- [ ] **TC-2 (json.rs test must be updated in Tasks 5 and 6, separately):**
  `json.rs:test_all_json_outputs_parse` needs two rounds of updates: first when Task 5
  adds fields to `SearchResults`, then when Task 6 adds `base_type` to `SchemaModel`.
  Each task's commit must include its corresponding `json.rs` test edit or the build
  breaks between tasks.

- [ ] **TC-3 (Tasks 3 and 4 both touch `text.rs` rendering but different functions):**
  Task 3 modifies the endpoint loop inside `render_search`. Task 4 modifies
  `render_param_section` and `render_fields_section`. These are distinct functions with
  no overlap. No conflict.

---

## Success criteria audit

| Task | Test | Fixture verified | Assertion correct |
|------|------|------------------|-------------------|
| T1 | `test_search_field_name_email` | User, CreateUserRequest, PatchUserRequest have `email` | [x] |
| T1 | `test_search_field_name_does_not_shadow_name_match` | "user" hits "User" by name | [x] |
| T2 | `test_expand_array_of_ref` | Error.details = ErrorDetail[]; ErrorDetail has field+reason | [x] |
| T3 | `test_search_endpoint_match_reason_parameter` | session_token is cookie param on GET /users | [x] |
| T3 | `test_search_endpoint_match_reason_none_for_path_match` | /users path contains "users" | [x] |
| T4 | `test_empty_path_params_section_suppressed` | POST /users has no path params | [x] |
| T4 | `test_empty_query_params_section_suppressed` | POST /users has no query params | [x] |
| T4 | `test_non_empty_path_params_still_shown` | GET /users/{userId} has userId path param | [x] |
| T4 | `test_constraint_column_alignment` | username min:3, password min:8 in CreateUserRequest | [x] |
| T5 | `test_schema_listing_drill_deeper_hint` (unit) | Already implemented — test is redundant | see T5-2 |
| T5 | `test_auth_drill_deeper_shows_scheme_search_hints` | bearerAuth scheme in model | [x] |
| T6 | `test_non_admin_role_has_base_type` | NonAdminRole uses `not:` | [x] |
| T6 | `test_non_admin_role_shows_base_type` | integration checks text output | [x] |

---

## Issue summary (ranked by severity)

| ID | Severity | Task | Description | Fix |
|----|----------|------|-------------|-----|
| T5-1 | **Blocking compile error** | T5 | `SchemaListModel` does not exist | Use `&[String]` directly |
| T6-1 | **Compile error** | T6 | `Type::Boolean { .. }` is wrong syntax for tuple variant | Use `Type::Boolean(_)` |
| T6-2 | **Silent wrong output** | T6 | `base_type` never reaches JSON output | Add field to `SchemaDetailJson` |
| T5-2 | **Redundant/duplicate** | T5 | Schema list drill-deeper already live; unit test name conflicts | Skip Step 4 implementation; keep integration test |
| T5-3 | **Build break between tasks** | T5 | `json.rs` test needs new `SearchResults` fields | Update in same commit |
| T6-3 | **Build break between tasks** | T6 | `json.rs` tests need `base_type: None` | Update in same commit |
| T3-1 | **Silent wrong behavior** | T3 | `desc_match` shadowing | Rename to `op_desc_match` (plan notes this) |
| T4-1 | **Test breakage risk** | T4 | Existing tests may assert `"Path Parameters:"` for empty-param endpoints | Run `cargo test` after first impl step |
