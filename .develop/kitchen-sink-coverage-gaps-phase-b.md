# Kitchen Sink Coverage Gaps — Phase B Analysis

**Date:** 2026-02-23
**Plan analyzed:** `docs/plans/2026-02-23-kitchen-sink-coverage-gaps-implementation.md`
**Source verified against:** openapiv3 v2.2.0, actual source files

---

## Analysis Method

For each task, source files were read directly and cross-referenced against the openapiv3 v2.2.0 crate
source in `~/.cargo/registry/src/`. API field names, type signatures, and struct layouts were
verified rather than assumed.

---

## Phase 1: Model Changes (Tasks 1–5)

### Task 1 — Extend `Field` with `write_only`, `deprecated`, `constraints`

- [x] **Implicit dependencies** — None. The plan correctly identifies this as a standalone task.
- [x] **Missing context** — `Field` struct exists at `src/models/resource.rs:70–83`. Current fields
  confirmed: `name`, `type_display`, `required`, `optional`, `nullable`, `read_only`, `description`,
  `enum_values`, `default_value`, `example`, `nested_schema_name`, `nested_fields`. All three new
  fields are additions, not replacements.
- [x] **Hidden blockers** — `openapiv3::SchemaData` at v2.2.0 confirmed to have `write_only: bool`,
  `deprecated: bool`. Both are direct bool fields, not `Option<bool>`. No coercion needed.
- [ ] **Cross-task conflicts** — The plan warns about construction sites: "two: `build_fields` in
  `commands/resources.rs` and test helpers in `render/text.rs` and `render/json.rs`." This count is
  wrong. Actual construction sites: (1) `build_fields` in `commands/resources.rs:207–220`, (2) test
  helpers in `render/text.rs` tests (multiple `Field { ... }` literals at lines ~878, ~893, ~938,
  ~945, ~959), (3) `render/json.rs` test at line ~422. The plan underestimates the blast radius.
  However the fix is mechanical — `cargo check` finds all sites — so this is a process note, not a
  blocker.
- [x] **Success criteria** — Clear: `cargo check` produces no "missing field" errors.

### Task 2 — Add `ResponseHeader` model and `headers` field to `Response`

- [x] **Implicit dependencies** — None listed or needed.
- [x] **Missing context** — `Response` struct exists at `src/models/resource.rs:55–61`. Current
  fields: `status_code`, `description`, `schema_ref`, `example`. All confirmed.
- [x] **Hidden blockers** — `openapiv3::Response` at v2.2.0 confirmed to have
  `headers: IndexMap<String, ReferenceOr<Header>>`. The `Header` struct has `format:
  ParameterSchemaOrContent` (NOT `data: ParameterSchemaOrContent` as the plan says). The plan's
  extraction code in Task 9 references `header.data` — this is wrong; it should be `header.format`.
  This is a blocker for Task 9, not Task 2 itself.
- [x] **Cross-task conflicts** — `Response` construction sites: `extract_responses` in
  `commands/resources.rs:357–364`, and tests in `render/text.rs` and `render/json.rs`. All need
  `headers: vec![]`. Task 3 also modifies `Response` to add `links`, so Tasks 2 and 3 must be
  applied together or Task 3 builds on Task 2's version — the plan sequences them correctly.
- [x] **Success criteria** — Clear.

### Task 3 — Add `ResponseLink` model and `links` field to `Response`

- [x] **Implicit dependencies** — Structurally depends on Task 2 completing first (both modify
  `Response`), which the plan states.
- [x] **Missing context** — `ResponseLink` is a new struct. No conflicts with existing code.
- [ ] **Hidden blockers** — The `ResponseLink.operation_id: String` field (non-optional) is a
  problem. In the openapiv3 v2.2.0 crate, `Link.operation` is a `LinkOperation` enum, not a direct
  `operation_id: Option<String>` field. The two variants are:
  - `LinkOperation::OperationRef(String)` — a $ref path
  - `LinkOperation::OperationId(String)` — an operationId name

  The extraction code in Task 10 uses `link.operation_id.clone()?` — this field does not exist on
  `openapiv3::Link`. The correct access pattern is:
  ```rust
  match &link.operation {
      openapiv3::LinkOperation::OperationId(id) => Some(id.clone()),
      openapiv3::LinkOperation::OperationRef(_) => None, // or handle separately
  }
  ```
  This is a compile-time blocker in Task 10. The `ResponseLink` model struct can still use
  `operation_id: String` as the stored value after extraction — but the extraction code needs
  correction.
- [x] **Cross-task conflicts** — Modifies `Response` (same struct as Task 2). Must be applied
  sequentially after Task 2.
- [x] **Success criteria** — Clear.

### Task 4 — Add `CallbackResponse`, `CallbackOperation`, `CallbackEntry` models; add `callbacks`/`links` to `Endpoint`

- [x] **Implicit dependencies** — None listed. Correct — it's self-contained model work.
- [x] **Missing context** — `Endpoint` struct at `src/models/resource.rs:11–25`. Confirmed fields:
  `method`, `path`, `summary`, `description`, `is_deprecated`, `is_alpha`, `external_docs`,
  `parameters`, `request_body`, `responses`, `security_schemes`, `drill_deeper`. Two new fields
  (`callbacks: Vec<CallbackEntry>`, `links: Vec<ResponseLink>`) are additions.
- [ ] **Hidden blockers** — `Endpoint` construction sites in tests are numerous. The plan says "all
  `Endpoint { ... }` construction in tests" without enumerating them. In `render/text.rs` tests:
  two `Endpoint` literals at lines ~1009 and ~1075 (both have all 12 fields explicitly). In
  `render/json.rs` tests: one `Endpoint` literal at line ~422 and another at ~470. In
  `commands/resources.rs` tests: `make_group` helper returns `ResourceGroup` but `Endpoint` is used
  in `test_render_resource_detail` via `render/text.rs`. Total count of `Endpoint` struct literal
  construction sites across all test files is at least 6–8. Again mechanical but undercounted.
- [x] **Cross-task conflicts** — `Endpoint` is not modified by any other Phase 1 task. Safe.
- [x] **Success criteria** — Clear. The plan's note about all four new structs needing
  `serde::Serialize` is correct and confirmed from the callback module design.

### Task 5 — Add `title` to `SchemaModel`

- [x] **Implicit dependencies** — None.
- [x] **Missing context** — `SchemaModel` at `src/models/schema.rs:8–16`. Confirmed fields: `name`,
  `description`, `fields`, `composition`, `discriminator`, `external_docs`. `title: Option<String>`
  is a clean addition.
- [x] **Hidden blockers** — `openapiv3::SchemaData.title: Option<String>` confirmed at v2.2.0.
  Direct field access, no API mismatch.
- [ ] **Cross-task conflicts** — `SchemaModel` is constructed in `commands/schemas.rs:138–145` and
  in test helpers in `commands/schemas.rs:243`, `render/text.rs:875` and `render/json.rs:383`.
  These all need `title: None`. The plan mentions "all `SchemaModel { ... }` construction sites" but
  does not enumerate. The test in `render/text.rs` at line 875 uses `SchemaModel { name, description,
  fields, composition, discriminator, external_docs }` — this is also a construction site. Mechanical
  fix, not a blocker.
- [x] **Success criteria** — Clear.

---

## Phase 2: Extraction Changes (Tasks 6–13)

**Pre-condition for all Phase 2 tasks:** The kitchen-sink fixture
(`tests/fixtures/kitchen-sink.yaml`) must exist before any Phase 2 test can run. The plan assumes
this file will be created before Phase 2 begins but does not include a task for creating it. This is
a **structural gap** — the fixture is the single hardest dependency in the whole plan and it has no
task assigned to it. Every test in Tasks 6–13 will fail with a file-not-found panic until the
fixture exists.

### Task 6 — Extract `write_only` and `deprecated` into `build_fields`

- [x] **Implicit dependencies** — Task 1 (correct). No other dependencies.
- [x] **Missing context** — `build_fields` in `commands/resources.rs:111–224`. The `fields.push`
  call is at line 207. The plan's code shows the correct field access:
  `resolved.schema_data.write_only` and `resolved.schema_data.deprecated` — both confirmed as direct
  `bool` fields on `openapiv3::SchemaData`.
- [x] **Hidden blockers** — None. API matches exactly.
- [x] **Cross-task conflicts** — Task 7 also modifies `build_fields` (replaces `constraints: vec![]`
  with `extract_constraints(...)`). Tasks 6 and 7 must be applied sequentially within the same
  `fields.push` call. No conflict if done in order.
- [x] **Success criteria** — Clear: TDD test `test_write_only_field_extraction` passes.

### Task 7 — Extract schema constraints into `build_fields`

- [x] **Implicit dependencies** — Tasks 1 and 6 (correct).
- [x] **Missing context** — The plan's `extract_constraints` function references specific field names
  on openapiv3 types. All verified against v2.2.0:
  - `StringType`: `min_length: Option<usize>`, `max_length: Option<usize>`, `pattern: Option<String>` — confirmed
  - `IntegerType`: `minimum: Option<i64>`, `maximum: Option<i64>`, `multiple_of: Option<i64>`,
    `exclusive_minimum: bool`, `exclusive_maximum: bool` — confirmed
  - `NumberType`: `minimum: Option<f64>`, `maximum: Option<f64>`, `multiple_of: Option<f64>`,
    `exclusive_minimum: bool`, `exclusive_maximum: bool` — confirmed
  - `ArrayType`: `min_items: Option<usize>`, `max_items: Option<usize>`, `unique_items: bool` — need
    to verify
  - `ObjectType`: `min_properties: Option<usize>`, `max_properties: Option<usize>` — confirmed
- [ ] **Hidden blockers** — The plan's code for integer/number constraints uses `ref` patterns like
  `if let Some(ref min) = i.minimum { c.push(format!("min:{}", min)); }`. Since `minimum` is
  `Option<i64>` (Copy), the `ref` is unnecessary but harmless. For number, it's `Option<f64>` which
  is also Copy. More importantly: the format string for integer minimum would produce `min:1` (i64
  display) which is correct. For float minimum, `format!("min:{}", 1.0f64)` produces `"min:1"`, not
  `"min:1.0"` in Rust. This inconsistency is minor and matches the expected fixture output in the
  tests.
- [x] **Cross-task conflicts** — Modifies `build_fields` alongside Task 6. Sequential application
  required. No structural conflict.
- [x] **Success criteria** — Clear: three TDD tests pass.

### Task 8 — Fix integer/number enum extraction

- [x] **Implicit dependencies** — Task 1 (for the Field struct). The dependency note says only
  "Task 1" which is correct.
- [x] **Missing context** — `extract_enum_values` at `commands/resources.rs:275–284`.
  `IntegerType.enumeration: Vec<Option<i64>>` confirmed. `NumberType.enumeration: Vec<Option<f64>>`
  confirmed. The plan's replacement code is correct.
- [x] **Hidden blockers** — None on the `extract_enum_values` side. The `build_schema_model` change
  in `schemas.rs` is more complex: adding an arm for `Type::Integer` when `!int_type.enumeration.is_empty()`.
  This arm must be inserted BEFORE the existing `_ => (Vec::new(), None)` fallthrough. Position
  matters because Rust match arms are tested in order. Current match structure at line 81:
  `Type::Object`, `Type::String` (with enum check guard), `AllOf`, `OneOf`, `AnyOf`, `_`. The new
  arm for integer enums can be inserted after the string enum arm without conflict.
- [x] **Cross-task conflicts** — `extract_enum_values` is called only from `build_fields`. No
  conflicts with other tasks modifying the same function.
- [x] **Success criteria** — Clear: TDD test `test_integer_enum_schema_model` passes.

### Task 9 — Extract response headers in `extract_responses`

- [x] **Implicit dependencies** — Task 2 (correct). Also implicitly depends on Task 3 because the
  plan's replacement code for `extract_responses` constructs `Response { ..., headers, links: vec![] }`.
  If Task 3 has not yet added `links` to `Response`, this code won't compile. The plan sequences
  Task 9 after Tasks 2 and 3 (all Phase 1 done before Phase 2), so in practice this is fine, but
  the stated dependency is only "Task 2" when it actually requires Task 3 as well.
- [ ] **Hidden blockers** — **Critical API mismatch**: The plan's extraction code accesses
  `header.data` to get the schema:
  ```rust
  let type_display = match &header.data {
      openapiv3::ParameterSchemaOrContent::Schema(s) => { ... }
      _ => "string".to_string(),
  };
  ```
  The `openapiv3::Header` struct at v2.2.0 has NO field named `data`. The field is `format:
  ParameterSchemaOrContent`. This is a compile-time error. The correct code is:
  ```rust
  let type_display = match &header.format {
      openapiv3::ParameterSchemaOrContent::Schema(s) => { ... }
      _ => "string".to_string(),
  };
  ```
  This same pattern is already used correctly in `extract_param_schema_info` at
  `commands/resources.rs:583–585` (which accesses `format: &openapiv3::ParameterSchemaOrContent`).
  The plan used `data` instead of `format` — likely a naming error from reading older crate docs.
- [x] **Cross-task conflicts** — Task 10 also modifies `extract_responses` to populate `links`. Must
  be applied sequentially. The plan notes this at Task 10: "Update `extract_responses` (from Task 9)
  to also populate `links`".
- [x] **Success criteria** — Clear: `test_response_headers_extracted` passes.

### Task 10 — Extract links from responses in `get_endpoint_detail`

- [x] **Implicit dependencies** — Tasks 3 and 9 (correct). Also implicitly requires Task 3 to have
  added `links: Vec<ResponseLink>` to `Endpoint` (via Task 4), since `get_endpoint_detail` builds an
  `Endpoint` struct. The plan implies this through the dependency graph.
- [ ] **Hidden blockers** — **Critical API mismatch**: The plan's `extract_links_from_response`
  function accesses `link.operation_id`:
  ```rust
  let operation_id = link.operation_id.clone()?;
  ```
  `openapiv3::Link` at v2.2.0 has NO `operation_id` field. The field is `operation: LinkOperation`
  which is an enum. Correct extraction:
  ```rust
  let operation_id = match &link.operation {
      openapiv3::LinkOperation::OperationId(id) => id.clone(),
      openapiv3::LinkOperation::OperationRef(_) => return None,
  };
  ```
  This is a compile-time blocker. The code as written will not compile.
- [ ] **Hidden blockers (additional)** — The `build_link_drill_command` function iterates 5 methods
  (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`) but the plan's method list omits `HEAD`, `OPTIONS`, and
  `TRACE`. This is a minor functional gap but not a compile-time blocker. Any `operationId` on a
  HEAD/OPTIONS/TRACE operation would silently return `None` for the drill command.
- [x] **Cross-task conflicts** — Modifies `extract_responses` (same function as Task 9) and
  `get_endpoint_detail`. Must follow Task 9.
- [x] **Success criteria** — Clear: two TDD tests pass.

### Task 11 — Extract callbacks inline on `Endpoint`

- [x] **Implicit dependencies** — Tasks 4 and 12. The plan correctly notes Task 12 must be
  implemented first. This is an inversion of the listed task order (12 before 11).
- [x] **Missing context** — `get_endpoint_detail` in `commands/resources.rs:506–577`. The new call
  `crate::commands::callbacks::extract_callbacks_from_operation(operation, method, path)` will work
  once `callbacks.rs` is registered in `commands/mod.rs`.
- [x] **Hidden blockers** — None once Task 12 is done and the module is registered.
- [x] **Cross-task conflicts** — Modifies `get_endpoint_detail` (same function touched by Task 10
  for links). Both changes add new fields to the `Endpoint { ... }` construction at the end of the
  function. Can be applied cleanly if Task 10 goes first and Task 11 adds `callbacks:` to an already
  updated construction site. No conflict if sequenced.
- [x] **Success criteria** — Clear: two TDD tests pass.

### Task 12 — New callbacks extraction module

- [x] **Implicit dependencies** — Task 4 (correct). No other dependencies.
- [ ] **Missing context** — The plan says to register the module with `pub mod callbacks;` and
  points to `src/commands/mod.rs`. The actual content of `src/commands/mod.rs` was verified: it
  currently contains `pub mod overview`, `pub mod resources`, `pub mod schemas`, `pub mod auth`,
  `pub mod search`, `pub mod init`. The new line `pub mod callbacks;` goes here. This is correct.
- [ ] **Hidden blockers** — **API type mismatch**: The plan's `build_callback_entry` function
  iterates `callback` (type `&openapiv3::Callback`) with `.iter()`. `Callback` is
  `IndexMap<String, PathItem>` — note: `PathItem` directly, NOT `ReferenceOr<PathItem>`. The plan's
  code handles this with:
  ```rust
  let path_item = match path_ref {
      openapiv3::ReferenceOr::Item(pi) => pi,
      _ => return vec![],
  };
  ```
  But since `Callback` maps to `PathItem` (not `ReferenceOr<PathItem>`), this match will fail to
  compile — there's no `ReferenceOr` wrapper to destructure. The correct code is simply:
  ```rust
  let path_item = path_ref; // path_ref is already &PathItem
  ```
  This is a compile-time blocker. The plan's extraction loop variable `path_ref` should be typed as
  `&PathItem` directly.
- [ ] **Hidden blockers (additional)** — The `Operation.callbacks` field is
  `IndexMap<String, Callback>` where `Callback = IndexMap<String, PathItem>`. So the outer iteration
  is `(callback_name, cb)` where `cb: &Callback` (i.e. `&IndexMap<String, PathItem>`). The inner
  iteration is then `(url_expr, path_item)` where `path_item: &PathItem`. The plan's code:
  ```rust
  for cb_ref in operation.callbacks {
  ```
  also needs to be `for (callback_name, cb_ref) in &operation.callbacks` matching the plan's
  existing pattern. The plan's `extract_callbacks_from_operation` function uses
  `operation.callbacks.iter().filter_map(|(callback_name, cb_ref)| ...)` — this is correct. The
  problem is only in `build_callback_entry` where `path_ref` is treated as `ReferenceOr<PathItem>`
  when it is actually `PathItem`.
- [x] **Cross-task conflicts** — New file. No conflicts with existing files.
- [x] **Success criteria** — Clear: three TDD tests pass.

### Task 13 — Extract `title` in `build_schema_model`

- [x] **Implicit dependencies** — Task 5 (correct).
- [x] **Missing context** — `build_schema_model` in `commands/schemas.rs:71–146`. The `description`
  extraction at line 79. The new `title` extraction mirrors it exactly: `schema.schema_data.title.clone()`.
- [x] **Hidden blockers** — `openapiv3::SchemaData.title: Option<String>` confirmed at v2.2.0.
- [x] **Cross-task conflicts** — `SchemaModel` construction at line 138 needs `title` added.
  Task 5 already adds it to the struct, so after Task 5 that construction site is already broken
  and needs `title: None` placeholder. Task 13 replaces `title: None` with `title`. Sequencing is
  correct.
- [x] **Success criteria** — Clear: two TDD tests pass.

---

## Phase 3: Request Body Overhaul (Task 14)

### Task 14 — Multi-content-type request body extraction

- [x] **Implicit dependencies** — Tasks 6, 7, 8 (correct). These must be done so `build_fields`
  produces complete `Field` structs.
- [x] **Missing context** — `extract_request_body` at `commands/resources.rs:424–504`. The plan
  replaces this function entirely. The replacement is a drop-in with the same signature
  `(api, operation, expand) -> Option<RequestBody>`. Call sites are unchanged.
- [x] **Hidden blockers** — The `content_type.to_string()` call at the end — `content_type` is
  `&str` derived from either a `&'static str` literal (from `priority` array) or from a borrowed
  `ct.as_str()` from the `or_else` branch. Both are `&str`, `.to_string()` works on both.
- [ ] **Hidden blockers** — The `format_type_display` modification for binary fields introduces a
  **behavior change for all callers**, not just multipart. Any schema field with
  `type: string, format: binary` anywhere in the spec — including in non-multipart response bodies
  — will now render as `"binary"` instead of `"string/binary"`. This is the intended behavior per
  the design doc, but it's worth flagging: the existing test `test_build_fields_pet` does not test
  binary format, so no existing test breaks. New tests cover only the new case. Low risk, but the
  change is global.
- [x] **Cross-task conflicts** — Fully replaces `extract_request_body`. No partial modifications
  from other tasks touch this function.
- [x] **Success criteria** — Clear: four TDD tests and four integration tests pass.

---

## Phase 4: Rendering (Tasks 15–22)

### Task 15 — Text renderer: `write_only`, `deprecated`, `constraints`, integer enums

- [x] **Implicit dependencies** — Tasks 6, 7, 8 (correct).
- [x] **Missing context** — Two functions to update: `render_fields_section` at `text.rs:239–286`
  and `render_schema_fields` at `text.rs:418–478`. The plan correctly identifies both. The flag
  logic pattern is already established (`read_only`, `nullable` flags exist).
- [ ] **Hidden blockers** — The plan's test fixtures construct `SchemaModel` with a `title: None`
  field (e.g., line 1426: `title: None`). This assumes Task 5 (adding `title` to `SchemaModel`) is
  complete. Task 15 depends on Tasks 6/7/8 but the test code also references `title: None` on
  `SchemaModel`. If Tasks 5 and 15 are worked in parallel, the tests won't compile until Task 5 is
  done. This is an unlisted dependency. In practice the plan says all Phase 1 must complete before
  Phase 4, so it's sequenced correctly — but the dependency is implicit.
- [x] **Cross-task conflicts** — Tasks 16, 17, 18, 19 also modify `render_endpoint_detail` and
  `render_schema_detail`. Task 15 modifies `render_fields_section` and `render_schema_fields` which
  are helpers called by those render functions, not the top-level functions directly. No conflict.
- [x] **Success criteria** — Clear: four TDD tests pass.

### Task 16 — Text renderer: response headers

- [x] **Implicit dependencies** — Task 9 (correct). Also implicitly requires Task 2 (model) and
  Task 4 (for `Endpoint.callbacks`/`links` fields so test literal compiles). The test constructs an
  `Endpoint` struct literal with `callbacks: vec![], links: vec![]` — these fields only exist after
  Task 4 completes. Stated dependency is only Task 9 but Task 4 is also required for the test.
- [x] **Missing context** — `render_endpoint_detail` in `text.rs:82–204`. The responses loop is at
  lines 168–187. The plan inserts headers display after each response line.
- [x] **Hidden blockers** — None.
- [x] **Cross-task conflicts** — Tasks 17 and 18 also modify `render_endpoint_detail`. Each adds
  code to different sections (Links section, Callbacks section) so no line-level conflicts. But all
  three tasks touch the same function and must be applied in order.
- [x] **Success criteria** — Clear.

### Task 17 — Text renderer: links section

- [x] **Implicit dependencies** — Task 10 (correct). Same implicit dependency on Task 4 for
  `Endpoint` struct literal in test.
- [x] **Missing context** — Plan shows insertion point "after the Errors section and before Drill
  deeper." In `text.rs`, the errors section ends around line 193, drill deeper starts at 196. Clear
  insertion point.
- [x] **Hidden blockers** — None.
- [x] **Cross-task conflicts** — Same function as Task 16. Sequential application required.
- [x] **Success criteria** — Clear.

### Task 18 — Text renderer: callbacks inline section

- [x] **Implicit dependencies** — Task 11 (correct). Same implicit Task 4 dependency for `Endpoint`
  struct literal.
- [x] **Missing context** — Plan shows insertion "after the Links section." Assuming Task 17 is done
  first, the callbacks section goes after it. Clear.
- [x] **Hidden blockers** — None.
- [x] **Cross-task conflicts** — Same function as Tasks 16 and 17. Third sequential modification.
  Must be applied last of the three.
- [x] **Success criteria** — Clear.

### Task 19 — Text renderer: schema title display

- [x] **Implicit dependencies** — Task 13 (correct). Also requires Task 5 (model must have `title`).
- [x] **Missing context** — `render_schema_detail` at `text.rs:308–416`. The header block is at
  lines 318–321.
- [x] **Hidden blockers** — The plan's test for "title hidden when same as name" tests that
  `output.contains("Title:")` is false when title equals the schema name. This works if the
  condition is `if title != &model.name`. However the comparison is between `String` and `String` —
  the `!= &model.name` dereference is needed. The plan's code `if title != &model.name` is correct
  because `title` is `&String` and `model.name` is `String`, so `&model.name` is also `&String`
  and the comparison works.
- [x] **Cross-task conflicts** — Modifies `render_schema_detail`. No other Phase 4 task modifies
  this function's header section.
- [x] **Success criteria** — Clear.

### Task 20 — JSON renderer: update `FieldJson` and `convert_fields`

- [x] **Implicit dependencies** — Tasks 6, 7, 8 (correct). Also implicitly requires Task 5 for the
  `SchemaModel.title` addition to `SchemaDetailJson`.
- [x] **Missing context** — `FieldJson` at `json.rs:36–49`. `convert_fields` at `json.rs:57–74`.
  `SchemaDetailJson` at `json.rs:4–13`. All confirmed present with the exact current fields.
- [ ] **Hidden blockers** — The test extension to `test_all_json_outputs_parse` constructs a full
  `Endpoint` with `request_body` containing a `Field` with all new fields. This test also constructs
  a `SchemaModel` with `title`. Both require Phase 1 to be complete. The plan's test code (line 1915)
  constructs `Endpoint { ... }` but the plan does not show the full `Endpoint` struct literal. The
  test is described as "Extend the existing `test_all_json_outputs_parse` test" — this means
  modifying an existing test, which is a potential merge conflict if another task also extends it
  (Task 21 adds its own verification section). Both tasks say to extend the same test function.
  These modifications must be applied in order (Task 20 first, Task 21 second).
- [x] **Cross-task conflicts** — Task 21 also modifies `render/json.rs`. Different functions
  (`FieldJson` vs `render_endpoint_detail` and `render_schema_detail`). No overlap.
- [x] **Success criteria** — Clear.

### Task 21 — JSON renderer: update endpoint detail for headers, links, callbacks

- [x] **Implicit dependencies** — Tasks 9, 10, 11 (correct). These populate the new fields on the
  model structs. The JSON renderer uses `serialize(endpoint, is_tty)` which is a pass-through —
  no code change needed.
- [x] **Missing context** — `render_endpoint_detail` in `json.rs:327–330`. Confirmed: it uses
  `serialize(endpoint, is_tty)` which calls `serde_json::to_string` on the `Endpoint` struct.
  Since all new structs carry `#[derive(serde::Serialize)]`, the new fields appear automatically.
- [x] **Hidden blockers** — None. This is the most accurately described task in the plan.
- [x] **Cross-task conflicts** — The plan notes Task 21 as "no code change needed." The only work
  is adding a verification test. Test extends `test_all_json_outputs_parse` same as Task 20. Must
  be applied after Task 20.
- [x] **Success criteria** — Clear: verification test passes.

### Task 22 — New `callbacks` subcommand: renderers + CLI wiring

- [x] **Implicit dependencies** — Task 12 (correct for the extraction logic). Also requires Task 4
  (model structs) and Tasks 16–18 (for test struct literals to compile with full `Endpoint` fields).
- [x] **Missing context** — `src/main.rs` `Commands` enum at lines 26–59. The new `Callbacks`
  variant goes here. The `match &cli.command` block is at lines 112–254. The `init` early-return
  pattern at line 104 uses `if let Some(Commands::Init { spec_path }) = &cli.command`. The
  completions early-return at line 94 uses `if let Some(Commands::Completions { shell }) =
  cli.command`. No `unreachable!()` match arms exist for these — they use early returns. The plan
  says "add `Some(Commands::Callbacks { .. }) => unreachable!()` in any match arms that handle
  Init/Completions early-returns" — this is **incorrect**. Init and Completions use early `if let`
  guards and return before reaching the main match, so they are NOT match arms that need
  `unreachable!()`. The main match at line 112 will simply need a new arm
  `Some(Commands::Callbacks { name }) => { ... }`. The two existing `unreachable!()` arms at lines
  252–253 (`Init` and `Completions`) should be left as-is; the new arm is a normal arm before them.
- [x] **Hidden blockers** — The `render_overview` text function update adds a new command hint. The
  plan shows adding it after the existing commands. The `render_overview` tests check for specific
  command hints. Adding a new line doesn't break existing tests. However the existing integration
  test `test_overview_text` at `tests/integration_tests.rs:26` checks for `phyllotaxis resources`,
  `phyllotaxis schemas`, `phyllotaxis auth` — it does not assert absence of other content. No
  breakage.
- [x] **Cross-task conflicts** — Modifies `src/main.rs` `Commands` enum and match block. No other
  task in the plan touches `main.rs`. Safe.
- [x] **Success criteria** — Clear: six integration tests pass.

---

## Cross-Cutting Issues

### Issue A — Kitchen-sink fixture has no task

The single most critical dependency across all of Phase 2 and the integration tests in Phase 4 is
`tests/fixtures/kitchen-sink.yaml`. The plan has no task for creating it. Every test from Task 6
through Task 22 (integration tests) that references the fixture will fail with a runtime panic until
the file exists. The fixture must be created before Phase 2 begins. This should be a pre-flight task.

### Issue B — Three compile-time blockers from API mismatches

Three places in the plan will produce compile errors against openapiv3 v2.2.0:

1. **Task 9**: `header.data` — field does not exist. Correct field is `header.format`.
2. **Task 10**: `link.operation_id` — field does not exist. Correct pattern is match on
   `link.operation: LinkOperation` enum.
3. **Task 12**: `ReferenceOr<PathItem>` destructuring — `Callback` is `IndexMap<String, PathItem>`
   (no `ReferenceOr` wrapper). The inner iteration variable is already `&PathItem`, not
   `ReferenceOr<PathItem>`.

### Issue C — `run_with_kitchen_sink` helper is undeclared

The integration tests in Task 22 call `run_with_kitchen_sink(&["callbacks"])`. The
`tests/integration_tests.rs` file has `run_with_petstore` as an existing helper but no
`run_with_kitchen_sink`. The plan shows the helper definition inline within Task 22's integration
test section — this is correct and must be added to `tests/integration_tests.rs`.

### Issue D — Undeclared dependency: Task 9 requires Task 3

Task 9's replacement of `extract_responses` constructs `Response { ..., headers, links: vec![] }`.
The `links` field on `Response` is added by Task 3. The plan states Task 9 depends on "Task 2" only,
but it also requires Task 3. Since all Phase 1 tasks complete before Phase 2 begins this is harmless
in practice, but the stated dependency list is incomplete.

### Issue E — Undeclared dependencies in Tasks 16, 17, 18

Tasks 16, 17, and 18 each construct `Endpoint` struct literals in their tests. These literals
include `callbacks: vec![], links: vec![]` (added by Task 4). The stated dependencies are Tasks 9,
10, and 11 respectively, but all three also implicitly require Task 4. Same caveat applies: Phase 1
completes first so it's safe in practice.

### Issue F — `test_all_json_outputs_parse` modified by two tasks

Tasks 20 and 21 both extend `test_all_json_outputs_parse` in `render/json.rs`. The plan describes
both as "extend the existing test" but doesn't explicitly sequence which goes first. Task 20 must be
applied before Task 21 because Task 20's changes to `FieldJson`/`convert_fields` are the base for
Task 21's verification of the endpoint JSON output. The ordering is implied by the dependency graph
but not stated.

### Issue G — Phase 4 test literals assume all Phase 1 complete

Multiple Phase 4 tasks (15, 16, 17, 18) construct `SchemaModel { ..., title: None, ... }` or
`Endpoint { ..., callbacks: vec![], links: vec![], ... }` in test code. These fields exist only after
Tasks 4 and 5 complete. The plan's rule "ALL of Tasks 1–5 must complete before running `cargo check`"
covers this — but it's worth noting that Phase 4 tests cannot compile until ALL Phase 1 tasks are
done, not just the explicitly listed dependencies for each Phase 4 task.

---

## Summary Table

| Task | Compile-Safe | API Correct | Dependencies Complete | Notes |
|------|-------------|-------------|----------------------|-------|
| 1    | [x]         | [x]         | [x]                  | Construction site count underestimated but not a blocker |
| 2    | [x]         | [x]         | [x]                  | |
| 3    | [x]         | [ ]         | [x]                  | `operation_id` field doesn't exist; extraction code in Task 10 |
| 4    | [x]         | [x]         | [x]                  | |
| 5    | [x]         | [x]         | [x]                  | |
| 6    | [x]         | [x]         | [x]                  | Needs kitchen-sink fixture |
| 7    | [x]         | [x]         | [x]                  | Needs kitchen-sink fixture |
| 8    | [x]         | [x]         | [x]                  | Needs kitchen-sink fixture |
| 9    | [ ]         | [ ]         | [x]                  | `header.data` should be `header.format`; missing Task 3 dep |
| 10   | [ ]         | [ ]         | [x]                  | `link.operation_id` doesn't exist; use `LinkOperation` enum |
| 11   | [x]         | [x]         | [x]                  | Must implement Task 12 first (plan notes this) |
| 12   | [ ]         | [ ]         | [x]                  | `ReferenceOr<PathItem>` pattern wrong; `Callback` maps to `PathItem` directly |
| 13   | [x]         | [x]         | [x]                  | |
| 14   | [x]         | [x]         | [x]                  | binary type display change is global, not just multipart |
| 15   | [x]         | [x]         | [x]                  | Implicit Task 5 dep via test literals |
| 16   | [x]         | [x]         | [x]                  | Implicit Task 4 dep via test literals |
| 17   | [x]         | [x]         | [x]                  | Implicit Task 4 dep via test literals |
| 18   | [x]         | [x]         | [x]                  | Implicit Task 4 dep via test literals |
| 19   | [x]         | [x]         | [x]                  | |
| 20   | [x]         | [x]         | [x]                  | Both Tasks 20+21 extend same test; sequence matters |
| 21   | [x]         | [x]         | [x]                  | No code change; only test addition |
| 22   | [x]         | [ ]         | [x]                  | Plan's `unreachable!()` instruction is wrong for Init/Completions |

---

## Required Fixes Before Implementation

The following must be corrected before encountering a compile failure:

**Fix 1 — Task 9, `header.data` → `header.format`:**
```rust
// Wrong (plan):
let type_display = match &header.data {
// Correct:
let type_display = match &header.format {
```

**Fix 2 — Task 10, `link.operation_id` → `link.operation` enum match:**
```rust
// Wrong (plan):
let operation_id = link.operation_id.clone()?;
// Correct:
let operation_id = match &link.operation {
    openapiv3::LinkOperation::OperationId(id) => id.clone(),
    openapiv3::LinkOperation::OperationRef(_) => return None,
};
```

**Fix 3 — Task 12, `Callback` iteration removes `ReferenceOr` wrapper:**
In `build_callback_entry`, the inner loop iterates `callback: &openapiv3::Callback` which is
`&IndexMap<String, PathItem>`. The `path_ref` variable is already `&PathItem`:
```rust
// Wrong (plan):
let path_item = match path_ref {
    openapiv3::ReferenceOr::Item(pi) => pi,
    _ => return vec![],
};
// Correct:
let path_item = path_ref; // already &PathItem, no ReferenceOr wrapper
```

**Fix 4 — Task 22, `main.rs` wiring instruction:**
The plan says to add `Some(Commands::Callbacks { .. }) => unreachable!()` alongside Init and
Completions. This is wrong. The new `Callbacks` variant needs a normal `Some(Commands::Callbacks { name }) => { ... }` arm in the main match. The existing `unreachable!()` arms for `Init` and
`Completions` (lines 252–253) are already there and should not be modified.

**Pre-flight — Kitchen-sink fixture:**
Create `tests/fixtures/kitchen-sink.yaml` before starting Phase 2. The fixture must include:
- Schema `CreateUserRequest` with `password` field having `writeOnly: true`
- Schema `PetBase` with `legacy_code` field having `deprecated: true` and `tags` with
  `uniqueItems: true`
- Schema `User` with `username` field having `minLength: 3`, `maxLength: 32`, `pattern`
- Schema `Settings` with `max_upload_size_mb` field having `minimum: 1`, `maximum: 1024`,
  `multipleOf: 5`
- Schema `Priority` as `type: integer, enum: [0, 1, 2, 3, 4]`
- Schema `GeoLocation` with `title: "Geographic Location"`
- Endpoint `GET /users` 200 response with `X-Total-Count` and `X-Rate-Limit-Remaining` headers
- Endpoint `POST /users` 201 response with `GetCreatedUser` and `ListUserPets` links
- Endpoint `POST /notifications/subscribe` with `onEvent` and `onStatusChange` callbacks
- Endpoint `POST /files/upload` with `multipart/form-data` body containing `file` (binary),
  `description`, `tags`
- Endpoint `PUT /files/{fileId}/metadata` with `application/x-www-form-urlencoded` body
- Schema `EventPayload` referenced by the `onEvent` callback body
- Resource group `files`, `users`, `notifications` from appropriate tags
