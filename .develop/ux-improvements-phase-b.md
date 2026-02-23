# Phase B Analysis: ux-improvements

Generated: 2026-02-23
Tasks analyzed: 6

---

## Task 1: Add `drill_deeper` to `Endpoint`, `schema_ref` to `RequestBody`

- [x] **Implicit deps:** None. This is the root task.
- [x] **Missing context:** The plan lists three literal sites to update after adding the fields (text.rs tests, json.rs test). It misses one production literal site: `extract_resource_groups` in `src/commands/resources.rs` (lines 56–68) constructs an `Endpoint` struct literal used for the lightweight group-listing path. It will fail to compile and must also get `drill_deeper: vec![]`. The plan's coverage of literal sites is incomplete.
- [x] **Hidden blockers:** None external. The compiler will surface every missed site as a build error, so TDD via `cargo test` catches them.
- [x] **Cross-task conflicts:** Task 1 touches `src/models/resource.rs` exclusively for the struct definition change. Tasks 2, 3, 4 touch different files and do not conflict with each other or with Task 1. The only ordering requirement is that Tasks 2/3/4 cannot compile until Task 1's struct changes are committed.
- [x] **Success criteria:** `cargo build` passes with no compile errors. `cargo test` passes with no failures. `Endpoint` has a `pub drill_deeper: Vec<String>` field. `RequestBody` has a `pub schema_ref: Option<String>` field. All `Endpoint { ... }` and `RequestBody { ... }` struct literals in the entire codebase compile, including the one in `extract_resource_groups` at `resources.rs:56`.

**Actions taken:**
- Documented the missing literal site in `extract_resource_groups`. The plan's "Struct literals to update" section lists only test sites; the production literal in `resources.rs` is unmentioned. Added it to the plan below.

---

## Task 2: Populate `drill_deeper` in `get_endpoint_detail`

- [x] **Implicit deps:** Depends on Task 1 (struct fields must exist). Stated correctly in the plan.
- [x] **Missing context:** The plan's Step 1 pseudocode references `schema_ref_name.clone()` as if it is already a named local variable in `extract_request_body`. It is not. In the current code, the `Reference` arm of the `match schema_ref` block (line 441–449) resolves the reference to get a `&Schema` but discards the `sname: &str` value. The implementer must capture that name before the match arm closes. Concretely: the `Reference` arm must be restructured to bind both the `sname` and the resolved schema, then store `Some(sname.to_string())` for use in the `RequestBody` constructor. The plan's pseudocode implies the name is already available, which will confuse an agent reading only the plan. The plan also does not address the `Item` arm: when the schema is inline (not a `$ref`), there is no schema name to capture; `schema_ref` stays `None`. This is correct behavior but needs to be stated explicitly.
- [x] **Hidden blockers:** None external. The petstore fixture covers the concrete-ref case (POST /pets uses `$ref: Pet`) and the oneOf case (POST /animals). The deduplication test requires a synthetic endpoint; the plan notes this and it is feasible with inline structs.
- [x] **Cross-task conflicts:** Task 2 only modifies `src/commands/resources.rs`. Tasks 3 and 4 only modify `src/render/text.rs` and `src/render/json.rs` respectively. No file overlap after Task 1.
- [x] **Success criteria:** All four test cases listed in the plan pass. `get_endpoint_detail` for `GET /pets/{id}` returns `drill_deeper == ["phyllotaxis schemas Pet"]`. `get_endpoint_detail` for `DELETE /pets/{id}` (204, no schema) returns `drill_deeper == []`. `get_endpoint_detail` for `POST /animals` (oneOf body with Pet/Owner options) returns `drill_deeper` containing both schema commands with no duplicates. `extract_request_body` for POST /pets sets `schema_ref: Some("Pet")` on the returned `RequestBody`.

**Actions taken:**
- Documented the missing `schema_ref_name` extraction context. Updated the plan's Task 2 Step 1 section with a clarifying note.

---

## Task 3: Text renderer — emit drill-deeper for endpoint detail

- [x] **Implicit deps:** Depends on Task 1 (the `drill_deeper` field must exist on `Endpoint`). Does not depend on Task 2 at compile time; the field just needs to exist and be accessible. Tests can use hardcoded `drill_deeper: vec![...]` literals. Stated correctly.
- [x] **Missing context:** None significant. `sanitize` is a private function in `text.rs`; using it in the new block is fine since it's in the same module. The insertion point ("after the existing Errors block") is unambiguous — the `Errors` section ends at line 194 of `text.rs`. The `is_tty` guard is correctly specified.
- [x] **Hidden blockers:** None. The existing `render_endpoint_detail` function in `text.rs` does not currently emit any "Drill deeper" block, so there is no existing code to conflict with.
- [x] **Cross-task conflicts:** Task 3 and Task 5 both modify `src/render/text.rs`, but they touch different functions: Task 3 touches `render_endpoint_detail` (line 82), Task 5 touches `render_search` (line 473). If run in parallel (e.g., two branches merged), a merge conflict is possible but not a semantic conflict — the changes are in separate functions and can be cleanly merged.
- [x] **Success criteria:** All three test cases pass. `render_endpoint_detail` with a non-empty `drill_deeper` and `is_tty = true` includes a "Drill deeper:" header and each command on its own line with two-space indent. With `is_tty = false`, no "Drill deeper:" section appears. With `drill_deeper: vec![]` and `is_tty = true`, no "Drill deeper:" section appears. Existing `test_render_endpoint_detail_post_pets` continues to pass (it uses `drill_deeper: vec![]` after the Task 1 struct update).

**Actions taken:**
- Noted the text.rs cross-task conflict with Task 5 (same file, different functions). No plan changes required — the sequencing table already flags this.

---

## Task 4: JSON renderer — assert `drill_deeper` in output

- [x] **Implicit deps:** Depends on Task 1 (the `drill_deeper` field must exist and derive `Serialize`). Does not depend on Task 2. The plan states this correctly. No code change to `json.rs` production code is needed — `serialize(endpoint, is_tty)` will automatically include the field once Task 1 is done.
- [x] **Missing context:** The plan says "No code change needed" for production code, which is accurate. However, it does not note that the `Endpoint` literal in `test_all_json_outputs_parse` (line 422–434 of `json.rs`) will be a compile error until `drill_deeper: vec![]` is added — this is Task 1's responsibility but lands in Task 4's test file. An agent working on Task 4 after Task 1 will find the file already compiling. If working concurrently, the Task 4 agent must add that field to the literal. The plan should flag this ordering sensitivity more explicitly.
- [x] **Hidden blockers:** None. The JSON serialization of `Vec<String>` to a JSON array is straightforward via serde derive.
- [x] **Cross-task conflicts:** Task 4 only touches `src/render/json.rs`. No other task modifies this file (Task 1 only updates a literal in the test section of `json.rs`). If Task 4 is worked before Task 1 fully lands, the agent may need to also add `drill_deeper: vec![]` to the existing endpoint literal in `test_all_json_outputs_parse`. No semantic conflict.
- [x] **Success criteria:** `test_all_json_outputs_parse` includes `assert!(v["drill_deeper"].is_array())` for the endpoint detail section and passes. The new test `test_endpoint_detail_json_includes_drill_deeper` passes: a `drill_deeper: vec!["phyllotaxis schemas Pet".to_string()]` endpoint serialized via `render_endpoint_detail` yields JSON where `v["drill_deeper"] == ["phyllotaxis schemas Pet"]`.

**Actions taken:**
- Noted the compile dependency on Task 1's literal update in `json.rs` test. No structural plan change needed, but the ordering sensitivity is documented here.

---

## Task 5: Search text renderer — emit full drill-down commands

- [x] **Implicit deps:** None stated and none actual. `EndpointMatch.resource_slug` is already populated (confirmed in `src/commands/search.rs` line 109–113). This is render-only.
- [x] **Missing context:** One subtlety: the existing `render_search` function already has a TTY-only "Drill deeper:" block (lines 517–531 of `text.rs`) that emits `phyllotaxis resources <slug>` and `phyllotaxis schemas <name>` commands for resources and schemas. Task 5's endpoint drill commands are emitted unconditionally (regardless of `is_tty`), as an inline second line per endpoint. These two blocks are structurally separate and do not conflict. However, the plan does not explain that the existing drill-deeper block remains untouched — an agent might wonder if the new per-endpoint lines replace or supplement the bottom-of-output block. The answer is: supplement. The plan could be clearer on this.
- [x] **Hidden blockers:** None. The `resource_slug` field already exists on `EndpointMatch` and is always populated (falls back to empty string if no tag, which the `!e.resource_slug.is_empty()` guard handles).
- [x] **Cross-task conflicts:** Same file conflict with Task 3 (both touch `src/render/text.rs`), but different functions (`render_search` vs `render_endpoint_detail`). If applied on separate branches, the merge is clean. No logical conflict.
- [x] **Success criteria:** All three test cases pass. A search result with `resource_slug: "pets"` emits `"    phyllotaxis resources pets GET /pets/{id}"` on the line immediately following the endpoint line. A result with `resource_slug: ""` does not emit the command. The command appears with `is_tty = false`. Existing search tests (`test_search_pet`, `test_search_no_results`, etc.) continue to pass — those test the search logic, not the renderer, but if any call `render_search` they will now include drill lines in output.

**Actions taken:**
- Documented the relationship between the new per-endpoint drill lines and the existing TTY drill-deeper block at the bottom of `render_search`. No plan edit required.

---

## Task 6: `PHYLLOTAXIS_SPEC` environment variable

- [x] **Implicit deps:** None. Fully independent of all other tasks.
- [x] **Missing context:** The insertion point is described clearly ("between the flag block and the config-file block"). In `src/spec.rs`, that is after line 98 (end of the `--spec` flag block) and before line 101 (start of the config-file block). The `bail!` macro and `PathBuf` are already imported. The `start_dir` parameter is available in scope. The plan is complete for implementation purposes. One note: `std::env::var` returns `Err` for unset variables and `Ok(val)` for set ones; the plan's empty-string check is correct and handles `export PHYLLOTAXIS_SPEC=""` gracefully.
- [x] **Hidden blockers:** Test isolation is the main hazard. `std::env::set_var` / `remove_var` mutate global process state and are inherently racy under Rust's parallel test runner. The plan flags this and recommends `--test-threads=1` if flakiness occurs. An alternative is `std::sync::Mutex`-based test serialization within the module (e.g., a static `ENV_MUTEX`), which avoids affecting all tests. Either approach works; the plan's `--test-threads=1` recommendation is the simpler path and is acceptable for a small test suite.
- [x] **Cross-task conflicts:** Task 6 only modifies `src/spec.rs`. No other task touches this file. Zero conflict risk.
- [x] **Success criteria:** All five test cases listed in the plan pass. `resolve_spec_path(None, &no_config, dir)` with `PHYLLOTAXIS_SPEC` set to a valid file path returns `Ok(that_path)`. With `PHYLLOTAXIS_SPEC` set and `--spec` flag also provided, the flag wins. With `PHYLLOTAXIS_SPEC` set and a config file present, env var wins over config. With `PHYLLOTAXIS_SPEC` pointing to a nonexistent path, `Err` is returned with a message containing `"PHYLLOTAXIS_SPEC"`. With `PHYLLOTAXIS_SPEC=""`, resolution falls through to config/auto-detect. All existing `resolve_spec_path` tests continue to pass (they do not set `PHYLLOTAXIS_SPEC`, so they are unaffected assuming env is clean in CI).

**Actions taken:**
- None required. Plan is sufficient for implementation.

---

## Plan updates made

### Task 1 — Missing literal site added

The plan's "Struct literals to update" bullet list omits one production site. Updated that section:

> **Struct literals to update:**
> - `src/commands/resources.rs` — `extract_resource_groups` function, ~line 56: add `drill_deeper: vec![]`
> - `src/render/text.rs` tests — `Endpoint` literals: add `drill_deeper: vec![]`; `RequestBody` literals: add `schema_ref: None`
> - `src/render/json.rs` tests — `Endpoint` literal: add `drill_deeper: vec![]`

### Task 2 — Clarified `schema_ref_name` extraction

Added a note in Step 1 clarifying that `schema_ref_name` does not exist as a variable in the current `extract_request_body` and must be extracted from the `Reference` match arm:

> **Implementation note:** In the current code, `extract_request_body` matches `schema_ref` (line 439) but the `Reference` arm resolves to a `&Schema` without retaining the name. Step 1 requires restructuring this arm to bind the name before resolving the schema. Concretely: save `sname.to_string()` as `Option<String>` before the match, then pass it to `RequestBody.schema_ref`. The `Item` arm (inline schema, no `$ref`) sets `schema_ref: None`.

---

## Summary

- Total tasks: 6
- Dependencies added: 0 (all stated dependencies were correct; no hidden cross-task deps found)
- New beads created: 0
- Plan updates made: 2 (missing literal site in Task 1; missing context note in Task 2)
- Success criteria added: 6 (one per task, all measurable)
