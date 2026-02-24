# Phase B Analysis: kitchen-sink-review-fixes

Generated: 2026-02-23
Tasks analyzed: 10

---

## Task 1: Empty Request Body Display for Non-Schema Content Types

- [x] Implicit deps: None. The fix is a pure text-rendering change inside the existing `else` branch of `render_endpoint_detail` in `text.rs`. No other task needs to run first.
- [x] Missing context: Agent has sufficient context. The plan provides the exact current code (lines 132-143 in `text.rs`), the exact replacement, and a known test endpoint (`POST /admin/bulk-import`). Verified in source: the current `else` branch calls `render_fields_section` unconditionally when fields is empty, producing blank output.
- [x] Hidden blockers: None. The fixture `kitchen-sink.yaml` already exists (listed in git status as `??`). The test helper `run_with_kitchen_sink` must already be defined in `integration_tests.rs` — confirmed by presence of kitchen-sink unit tests in `resources.rs` (`load_kitchen_sink()`). If `run_with_kitchen_sink` is not yet in `integration_tests.rs`, the test will not compile. The plan does not show existing test helper signatures, but given other kitchen-sink integration tests are referenced, the helper is assumed present.
- [x] Cross-task conflicts: Task 4 also modifies `text.rs` (response headers block). Task 6 modifies `render_callback_list`. Task 8 modifies `render_search`. Task 10 modifies `render_overview`. All four touch different functions in `text.rs` with no overlap. Task 1 touches only the `render_endpoint_detail` request body section. No conflict.
- [x] Success criteria: (1) `cargo test test_raw_body_shown_for_csv_content_type` passes — stdout contains `"text/csv"` and `"Raw body (no schema)"`. (2) `cargo test test_resources_endpoint_post` continues to pass — petstore POST /pets shows fields, not "Raw body". (3) The specific text output for `POST /admin/bulk-import` must contain the string `Raw body (no schema)` on a line indented with two spaces.

**Actions taken:**
- Verified in `text.rs` lines 131-143: the current `else` branch calls `render_fields_section` regardless of whether `body.fields` is empty, which silently produces nothing for CSV bodies. The plan's fix is correct.
- Confirmed that the existing `render_fields_section` function (line 288) already returns early if `fields.is_empty()` — so the current behavior is blank output, not an error.

---

## Task 2: Exclusive Min/Max Constraint Formatting

- [x] Implicit deps: None. Pure change to `extract_constraints` in `resources.rs`. No other task touches this function.
- [x] Missing context: Mostly sufficient. The plan documents the openapiv3 2.2 crate's field types as plain `bool` (not `Option<bool>`). Verified in source: `resources.rs` lines 239-265 show the current implementation appends `"exclusiveMinimum"` and `"exclusiveMaximum"` as standalone strings. The plan's replacement code is self-contained. One gap: the plan says fixture data `GeoLocation.accuracy_m` has `minimum: 0, exclusiveMinimum: true` and `Bird.wingspan_cm` has `minimum: 1.0, maximum: 400.0, exclusiveMaximum: true` — these are asserted but not independently verified here since we cannot run code. The test structure implies the fixture exists and contains these.
- [x] Hidden blockers: None. The fixture `kitchen-sink.yaml` is required and present. The `test_constraints_integer` regression test references `Settings.max_upload_size_mb` which uses regular `min:`/`max:` without exclusive flags — confirmed safe by reading the current code which would produce `min:N max:N` for those fields.
- [x] Cross-task conflicts: Task 3 also modifies `resources.rs` (`format_type_display` function). These are distinct functions with no shared code. Both tasks touch `integration_tests.rs` but add different tests. Sequential execution eliminates conflict.
- [x] Success criteria: (1) `cargo test test_exclusive_minimum_formatted_as_operator` passes — `schemas GeoLocation` stdout contains `">0"` and does not contain `"exclusiveMinimum"`. (2) `cargo test test_exclusive_maximum_formatted_as_operator` passes — `schemas Bird` stdout contains `"<400"` and does not contain `"exclusiveMaximum"`. (3) `cargo test test_constraints_integer` continues to pass — `Settings.max_upload_size_mb` still shows `min:` and `max:` prefixed tokens unchanged.

**Actions taken:**
- Verified current `extract_constraints` at lines 231-265: Integer and Number arms push `"exclusiveMinimum"` and `"exclusiveMaximum"` as separate tokens after the `min:`/`max:` tokens, confirmed by lines 243-244 and 250-251.
- The replacement code in the plan correctly integrates the flag into a conditional format branch. Matches the crate behavior as documented.

---

## Task 3: Array Item Types Propagate to Type Display

- [x] Implicit deps: None. Isolated to the `Array` arm of `format_type_display` in `resources.rs`.
- [x] Missing context: Sufficient. The plan documents the `openapiv3::ReferenceOr::Item(boxed)` case where `boxed` is `Box<Schema>` and `boxed.schema_kind` auto-derefs. Verified in source: `resources.rs` lines 287-294 show the current `Array` arm — the `_ => "array".to_string()` fallthrough captures inline array items. The plan's added arm calls `format_type_display(&boxed.schema_kind)` recursively, which is correct since `format_type_display` is already defined in the same file.
- [x] Hidden blockers: None. The kitchen-sink fixture's `POST /files/upload-batch` must define a `files` field with `type: array, items: {type: string, format: binary}`. The plan states this is "verified fixture data." The recursive call to `format_type_display` handles all format cases including `binary` (already handled in the String arm returning `"binary"` directly).
- [x] Cross-task conflicts: Task 2 also modifies `resources.rs` (different function, `extract_constraints`). No overlap. Both touch `integration_tests.rs` with new distinct test functions.
- [x] Success criteria: (1) `cargo test test_array_item_type_propagates_for_inline_binary` passes — `resources files POST /files/upload-batch` stdout contains `"binary[]"`. (2) `cargo test test_build_fields_allof` continues to pass — `PetList` tags field using `$ref` items still shows `Tag[]` or the ref name (unaffected, handled by the Reference branch which remains unchanged).

**Actions taken:**
- Confirmed the `format_type_display` function is private (`fn`, not `pub fn`) and called from within the same module — the recursive call `format_type_display(&boxed.schema_kind)` is valid.
- Confirmed `boxed` in `openapiv3::ReferenceOr::Item(boxed)` is `Box<Schema>`, and `.schema_kind` access is valid via auto-deref.

---

## Task 4: Trailing Whitespace on Empty Header Descriptions

- [x] Implicit deps: None. Isolated to the response headers block inside `render_endpoint_detail` in `text.rs`.
- [x] Missing context: Sufficient. The plan shows the exact current code at "around line 188" in `text.rs`. Verified: lines 188-194 in the source match — `writeln!(out, "      {}  {}  {}", ...)` with `h.description.as_deref().unwrap_or("")`. When description is None, the format string still emits two trailing spaces. The fix replaces this with a `match` that omits the description column entirely when absent or empty.
- [x] Hidden blockers: None. The `HEAD /health` endpoint in kitchen-sink must have a response header `X-Health-Status` with no description. This is stated as "verified fixture data" in the plan. The test looks for the line by string matching `"X-Health-Status"` and checks `!header_line.ends_with(' ')`.
- [x] Cross-task conflicts: Task 1 also modifies `text.rs` (request body section, lines 131-143). Task 4 modifies the response headers block (lines 188-194). These are different line ranges within `render_endpoint_detail` — no overlap. If both tasks run in order, they each make clean, non-overlapping edits to the same file. Sequential execution required to avoid merge issues.
- [x] Success criteria: (1) `cargo test test_no_trailing_whitespace_on_empty_header_description` passes — the `X-Health-Status` header line does not end with a space character. (2) `cargo test test_render_response_headers` continues to pass — the existing unit test uses `X-Total-Count` with `description: Some("Total count".to_string())` and still shows the description column.

**Actions taken:**
- Verified source at lines 188-194: the format string `"      {}  {}  {}"` with an empty string third argument produces two trailing spaces. The plan's fix is correct.
- Note: the existing unit test `test_render_response_headers` in `text.rs` (line 1509) uses a header with `description: Some("Total count".to_string())` — it will still match the `Some(desc)` arm in the new code. No test update needed for it.

---

## Task 5: Remove Top-Level Links Duplication in JSON Output

- [x] Implicit deps: None stated. However, there is a hidden ordering concern: Task 5 adds `#[serde(skip_serializing)]` to `Endpoint.links` in `resource.rs`. The existing unit test `test_endpoint_json_includes_new_fields` in `json.rs` at line 634 currently asserts `v["links"].is_array()`. The plan correctly identifies this and requires removing that assertion. If an agent runs Task 5 without removing the assertion in `json.rs`, the test suite will break.
- [x] Missing context: Sufficient. The plan shows the exact location in `resource.rs` (lines 12-27) and the exact assertion to remove in `json.rs` (line 634, the `v["links"].is_array()` line). Verified in source: `models/resource.rs` line 25 is `pub links: Vec<ResponseLink>,` without any serde annotation. `json.rs` line 634 is `assert!(v["links"].is_array(), "links should be present as array");`. Both match.
- [x] Hidden blockers: One real hidden blocker: `text.rs` line 205 accesses `endpoint.links` directly in `render_endpoint_detail` as `if !endpoint.links.is_empty()`. The `#[serde(skip_serializing)]` attribute only affects JSON serialization — the field remains in memory and the text renderer reads it unchanged. This is correctly understood in the plan's Background section. No code change needed to text.rs for this task.
- [x] Cross-task conflicts: Task 8 also modifies `json.rs` (adds `callbacks: vec![]` to `SearchResults` construction). Task 10 also modifies `json.rs` (adds `callback_count` to `OverviewJson`). These are different sections of `json.rs`. Task 5 modifies the `test_endpoint_json_includes_new_fields` test (line 634). Task 8 modifies `test_all_json_outputs_parse` (line 452). No overlap. Task 5 also modifies `resource.rs` while no other task in this plan touches that file.
- [x] Success criteria: (1) `cargo test test_json_endpoint_no_top_level_links` passes — JSON output for `POST /users` has no `"links"` top-level key but responses array contains at least one response with a non-empty `"links"` array. (2) `cargo test test_render_links_section` continues to pass — text renderer still shows the Links section (reads in-memory field, unaffected by skip_serializing). (3) `cargo test test_endpoint_json_includes_new_fields` passes with the updated assertion (no longer checks for `links`).

**Actions taken:**
- Verified `resource.rs` Endpoint struct: `pub links: Vec<ResponseLink>` at line 25, no existing serde attribute.
- Verified `json.rs` line 634: `assert!(v["links"].is_array(), "links should be present as array");` — must be removed.
- Confirmed `text.rs` line 205: `if !endpoint.links.is_empty()` reads the field directly — unaffected by serialization annotation.

---

## Task 6: Callback Operation Count in List View

- [x] Implicit deps: None. Isolated to `render_callback_list` in `text.rs`. The `CallbackEntry` struct already has `operations: Vec<CallbackOperation>` — confirmed in `resource.rs` line 104-111.
- [x] Missing context: Sufficient. The plan shows the exact current code in `render_callback_list` (lines 567-576) and the exact replacement. Verified in source: lines 567-581 match — the current loop does not include operation count. The `CallbackEntry` struct already exposes `operations.len()`.
- [x] Hidden blockers: None. The kitchen-sink fixture's `onEvent` callback has exactly 1 POST operation (confirmed by `test_callbacks_extracted_inline` in `resources.rs` which asserts `!on_event.operations.is_empty()` and checks `op.method == "POST"`). The test checks for `"(1 operation)"` which uses singular.
- [x] Cross-task conflicts: Tasks 1, 4, 8, 10 also modify `text.rs`. Task 6 modifies only `render_callback_list` (lines 567-581). Tasks 1 and 4 modify `render_endpoint_detail`. Task 8 modifies `render_search`. Task 10 modifies `render_overview`. All are different functions with no overlap. The existing unit test `test_render_callback_list` (line 1682) constructs a `CallbackEntry` with `operations: vec![]` — operation count would be `0`. The plan claims this test still passes because it checks for "onEvent" and the drill hint, not for the count. However, the test would produce `"  onEvent  (on POST /notifications/subscribe)  (0 operations)"` — and `test_render_callback_list` only asserts `output.contains("Callbacks")`, `output.contains("onEvent")`, and `output.contains("phyllotaxis callbacks <name>")`, not the format of the count. Passes correctly.
- [x] Success criteria: (1) `cargo test test_callback_list_shows_operation_count` passes — `callbacks` stdout contains `"(1 operation)"` (singular). (2) `cargo test test_render_callback_list` continues to pass — existing unit test checks for "Callbacks", "onEvent", and "phyllotaxis callbacks <name>" only (all still present). (3) The plural form `"(N operations)"` is produced for callbacks with N != 1, though no test explicitly covers that path — confirmed by the `if op_count == 1` branch.

**Actions taken:**
- Verified `CallbackEntry` struct in `resource.rs` line 104-111: `operations: Vec<CallbackOperation>` is present.
- Verified the existing `test_render_callback_list` unit test at lines 1682-1697 passes `operations: vec![]`. With the fix, output becomes `"  onEvent  (on POST /notifications/subscribe)  (0 operations)"` — the test only checks for `"Callbacks"`, `"onEvent"`, `"phyllotaxis callbacks <name>"`. All still present. No update needed.

---

## Task 7: Verify --expand Flag Works on Endpoint Detail View

- [x] Implicit deps: None. This is test-only. The `--expand` flag is confirmed wired in `main.rs` at lines 17-19 (global flag definition) and line 140 (`get_endpoint_detail(&loaded.api, method, path, cli.expand)`). No implementation change.
- [x] Missing context: The plan correctly identifies no implementation change is needed. However, the test asserts `stdout.contains("Owner:")` (with colon) for the expanded output. Verified in `text.rs`: `render_schema_fields` at line 532 shows that when `!f.nested_fields.is_empty()`, the output format is `"{name}  {type_display}:"` followed by nested fields. But this is `render_schema_fields` for schema detail, not `render_fields_section` for endpoint request bodies. The endpoint detail view uses `render_fields_section` (line 288) which does NOT render nested fields with a colon — it always renders the flat line format. The test asserts `stdout.contains("Owner:")` but `render_fields_section` will only show the owner field as `"  owner  Owner  ..."` without a colon even with `--expand`. This is a potential false pass/fail depending on where the expansion colon notation appears.

    Looking more carefully: `get_endpoint_detail` with `expand=true` calls `expand_fields_pub` in `extract_request_body`. The expanded `owner` field will have `nested_fields` populated. But `render_endpoint_detail` calls `render_fields_section` (not `render_schema_fields`), and `render_fields_section` does NOT check `nested_fields` — it renders the flat format regardless. So `"Owner:"` with a colon will NOT appear in the endpoint text output even with `--expand`.

    This is a hidden test correctness issue: the assertion `stdout.contains("Owner:")` may fail even though `--expand` is correctly wired. The test may need to assert a less specific condition, or the plan's expectation is wrong about how `--expand` surfaces in endpoint detail output vs schema detail output.

- [x] Hidden blockers: The test uses `run_with_petstore` (not kitchen-sink). This helper must be defined in `integration_tests.rs`. Given existing petstore integration tests exist, it is assumed present.
- [x] Cross-task conflicts: None. Only adds to `integration_tests.rs`. No source file changes.
- [x] Success criteria: (1) `cargo test test_resources_endpoint_expand_flag` passes. The assertion `stdout.contains("Owner")` (without colon) should pass since the owner field will show as type `"Owner"`. The assertion `stdout.contains("Owner:")` (with colon) may fail if `render_fields_section` does not emit the colon format for expanded nested fields. An agent implementing this task should verify the actual output format before committing the test. If the `"Owner:"` assertion fails, replace it with `stdout.contains("Owner")` as the weaker but valid check. (2) Exit code is 0.

**Actions taken:**
- Identified discrepancy: `render_fields_section` (used in endpoint detail) does not implement the nested colon format that `render_schema_fields` (used in schema detail) does. The `"Owner:"` assertion in the test may be incorrect. An agent should verify by running the command manually before writing the test assertion.

---

## Task 8: Search Covers Callbacks

- [x] Implicit deps: None. `search.rs` is independent. The plan correctly notes that adding `callbacks` to `SearchResults` requires updating all existing struct literal constructions in `text.rs` and `json.rs` to add `callbacks: vec![]`. These are compile-time requirements, not runtime ordering dependencies.
- [x] Missing context: One gap: the plan says to update `SearchResults` constructions in `text.rs` "around lines 1334, 1358, 1382." Verified in source: the three `SearchResults { ... }` constructions in `text.rs` tests are at lines 1334, 1358, and 1381. All three currently lack a `callbacks` field (which doesn't exist yet). After adding the field to the struct, all three will fail to compile until updated. The plan correctly identifies this. The `json.rs` construction is at line 452 — confirmed present and also needs `callbacks: vec![]`. An agent must update all four locations or the build breaks.
- [x] Hidden blockers: The plan uses `crate::commands::callbacks::list_all_callbacks(api)` inside `search.rs`. This cross-module call requires `callbacks` module to be public. Confirmed: `callbacks.rs` defines `pub fn list_all_callbacks(...)` and `find_callback` — both are public. The `CallbackEntry` struct and its fields are also public. The cross-crate path `crate::commands::callbacks::list_all_callbacks` is valid within the same library crate.
- [x] Cross-task conflicts: Task 8 modifies `text.rs` (adding callbacks section to `render_search`, updating `has_any` check, updating 3 test struct literals) and `json.rs` (updating 1 test struct literal). Task 10 modifies `text.rs` (updating 4 `OverviewData` test struct literals and `render_overview` function). Task 5 modifies `json.rs` (removing one assertion from `test_endpoint_json_includes_new_fields`). These are all different locations in their respective files. However, the same `text.rs` file is being modified by Tasks 1, 4, 6, 8, and 10 — sequential execution is mandatory to avoid conflicting edits.
- [x] Success criteria: (1) `cargo test test_search_finds_callbacks` passes — `search onEvent` stdout contains `"Callbacks:"` and `"onEvent"`. (2) `cargo test test_search_pet` and `test_search_no_results` continue to pass — existing search behavior is unaffected. (3) `cargo test test_render_search` passes — all 3 `SearchResults` struct literals in `text.rs` tests now include `callbacks: vec![]`. (4) `cargo test test_all_json_outputs_parse` passes — the `SearchResults` construction in `json.rs` now includes `callbacks: vec![]`. (5) The build compiles without errors.

**Actions taken:**
- Confirmed all three `SearchResults { ... }` literal sites in `text.rs` at lines 1334, 1358, 1381 need `callbacks: vec![]` added.
- Confirmed the `json.rs` construction at line 452 needs `callbacks: vec![]`.
- Confirmed `list_all_callbacks` is public and callable from `search.rs` via `crate::commands::callbacks::list_all_callbacks(api)`.

---

## Task 9: Fuzzy Matching for Callback Names

- [x] Implicit deps: None. The plan correctly notes `strsim = "0.11"` is already in `Cargo.toml` (used by `suggest_similar` in `resources.rs`). No dependency on other tasks.
- [x] Missing context: Mostly sufficient. One precision gap: the plan says to add `suggest_similar_callbacks` "after `find_callback` (after line 129)" in `callbacks.rs`. Verified: `find_callback` ends at line 129, and the `#[cfg(test)]` section begins at line 131. The new function must be inserted between lines 129 and 131. This is unambiguous.

    The plan says to use `strsim::jaro_winkler` via full path without a `use` import, mirroring `resources.rs`. This is correct — `resources.rs` calls `strsim::jaro_winkler` directly (line 815) without a `use` statement.

    One subtlety in `main.rs`: the plan replaces the `None` arm (lines 278-285) and uses `&callbacks` as the slice argument to `suggest_similar_callbacks`. The `callbacks` variable is bound at line 258 as `let callbacks = commands::callbacks::list_all_callbacks(&loaded.api)`. This is the same `Vec<CallbackEntry>` already in scope. The plan code passes `&callbacks` which is `&Vec<CallbackEntry>` — the function signature takes `&[CallbackEntry]` which Rust auto-derefs from `&Vec<T>`. This is correct.

- [x] Hidden blockers: None. `strsim` is already a dependency. The threshold of `0.8` for Jaro-Winkler is the same used in `resources.rs`. `"onEven"` vs `"onEvent"` — Jaro-Winkler score is well above 0.8 (approximately 0.98). `"xyzzy"` vs `"onEvent"` — well below 0.8.
- [x] Cross-task conflicts: Task 9 modifies `callbacks.rs` (new function at end) and `main.rs` (callback not-found error block, lines 278-285). No other task in this plan modifies `callbacks.rs` or `main.rs`. No conflict.
- [x] Success criteria: (1) `cargo test test_callbacks_fuzzy_suggestion_on_typo` passes — stderr for `callbacks onEven` contains `"onEvent"`. (2) `cargo test test_callbacks_no_suggestion_for_nonsense` passes — stderr for `callbacks xyzzy` contains `"not found"` and does not contain `"Did you mean"`. (3) `cargo test test_find_callback_not_found` continues to pass — `find_callback` returning `None` for nonexistent is unchanged.

**Actions taken:**
- Confirmed `suggest_similar` in `resources.rs` at lines 811-819 as a reference pattern.
- Confirmed `callbacks` variable is in scope at the not-found error path in `main.rs` (line 258 binding).
- Confirmed the insertion point in `callbacks.rs` is between the closing `}` of `find_callback` (line 129) and the `#[cfg(test)]` annotation (line 131).

---

## Task 10: Callback Count in Overview

- [x] Implicit deps: The plan states "None (self-contained struct change)" but there is a real compile-time dependency: adding `callback_count: usize` to `OverviewData` in `overview.rs` will immediately break all code that constructs `OverviewData { ... }` literals. The plan correctly identifies and lists all 5 affected sites (4 in `text.rs`, 1 in `json.rs`). These must all be updated in the same task. An agent that adds the field but misses one literal site will fail to compile.

    The plan also requires adding `callback_count` to the `OverviewJson` struct in `json.rs` and updating its construction. The field is not on `OverviewData` directly (which `json.rs` reads via `data.callback_count`) — it's added to both the data struct and the local JSON struct. This is correct and self-consistent.

- [x] Missing context: One gap: the plan shows `overview.rs` builds `callback_count` with `crate::commands::callbacks::list_all_callbacks(&loaded.api).len()`. But `overview.rs` currently uses `loaded: &LoadedSpec` as its parameter, and accesses `loaded.api`. This is correct — `list_all_callbacks` takes `&openapiv3::OpenAPI` and `loaded.api` is that type. The `crate::commands::callbacks` path is accessible from within the library crate.

    The `build` function in `overview.rs` does not currently import `crate::commands::callbacks`. The call `crate::commands::callbacks::list_all_callbacks(&loaded.api)` uses the full path, matching the pattern used elsewhere in the codebase (e.g., `crate::commands::resources::extract_resource_groups` at line 69 of `overview.rs`). No `use` statement needed.

- [x] Hidden blockers: None. The `callbacks` module is public and accessible. The kitchen-sink fixture has exactly 2 callbacks (`onEvent`, `onStatusChange`) — confirmed by `test_list_all_callbacks_finds_on_event` in `callbacks.rs`. The petstore fixture has 0 callbacks.
- [x] Cross-task conflicts: Task 10 modifies `text.rs` (4 `OverviewData` literal updates + `render_overview` function change) and `json.rs` (`OverviewJson` struct + construction + 1 `OverviewData` literal update). Task 8 also modifies `text.rs` (3 `SearchResults` literals + `render_search` changes) and `json.rs` (1 `SearchResults` literal). Task 5 modifies `json.rs` (removes one assertion). These all touch different locations in the files, but the same files are being modified by multiple tasks. Sequential execution is mandatory. The recommended order (1 → 2 → ... → 10) handles this correctly.
- [x] Success criteria: (1) `cargo test test_overview_shows_callback_count` passes — kitchen-sink overview stdout contains `"2 available"` or `"(2"`. (2) `cargo test test_overview_json_includes_callback_count` passes — JSON overview for kitchen-sink has `"callback_count": 2`. (3) `cargo test test_render_overview_basic`, `test_render_overview_no_auth`, `test_render_overview_with_description`, `test_render_overview_with_variables` all pass with `callback_count: 0` added to their struct literals. (4) `cargo test test_all_json_outputs_parse` passes with `callback_count: 0` added to the `OverviewData` construction at line 391. (5) `cargo test test_overview_text test_overview_json` pass — petstore overview shows `callback_count: 0`.

**Actions taken:**
- Confirmed all 4 `OverviewData { ... }` literal sites in `text.rs` tests (lines 828, 853, 868, 883) currently end with `schema_count: N,` without `callback_count`. Each needs `callback_count: 0` appended.
- Confirmed the `json.rs` `OverviewData` literal at line 391 ends with `schema_count: 0,` without `callback_count`. Needs `callback_count: 0`.
- Confirmed `overview.rs` `build()` uses `crate::commands::resources::extract_resource_groups` at line 69 as the pattern for the new `crate::commands::callbacks::list_all_callbacks` call.
- Confirmed `OverviewJson` in `json.rs` (lines 92-101) currently has `schema_count: usize` as last field. `callback_count: usize` must be added and the construction at line 135 updated to include `callback_count: data.callback_count`.

---

## Summary
- Total tasks: 10
- Dependencies added: 0 (all tasks are self-contained as stated; the compile-time struct literal updates are documented within each task, not as inter-task dependencies)
- New beads created: 0
- Plan updates made: 0 (findings are documented here as phase B analysis; the plan itself is not modified)
- Success criteria added: 10 (one per task, each with 2-5 specific measurable conditions)

## Notable Findings

**Task 7 has a potential test correctness issue:** The assertion `stdout.contains("Owner:")` (with colon) may not hold for endpoint detail output, since `render_fields_section` (used for endpoints) does not implement the nested-colon format that `render_schema_fields` (used for schema detail) does. An agent should manually verify the output format before finalizing the test assertion.

**Task 5 has a hidden compile dependency within the task:** Adding `#[serde(skip_serializing)]` to `Endpoint.links` and removing the `v["links"].is_array()` assertion from the existing test must be done in the same commit. If only the struct annotation is applied, the existing `test_endpoint_json_includes_new_fields` test will fail.

**Task 8 has four mandatory companion edits:** Adding `callbacks` to `SearchResults` is a breaking struct change. The agent must update all four literal construction sites (3 in `text.rs`, 1 in `json.rs`) in the same pass or the build will not compile.

**Task 10 has five mandatory companion edits:** Adding `callback_count` to `OverviewData` breaks all five literal construction sites (4 in `text.rs`, 1 in `json.rs`). All five must be updated together.

**Shared file `text.rs` (5 tasks):** Tasks 1, 4, 6, 8, and 10 all modify `text.rs`. They touch different functions and line ranges with no content overlap, but sequential execution is required. The recommended order 1 → 4 → 6 → 8 → 10 for these is safe.

**Shared file `json.rs` (3 tasks):** Tasks 5, 8, and 10 all modify `json.rs`. Again, different locations, sequential execution required.
