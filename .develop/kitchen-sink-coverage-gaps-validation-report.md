# Validation Report
## Design↔Plan Parity: kitchen-sink-coverage-gaps
**Attempt:** 2/5 — ✅ PASSED
**Date:** 2026-02-23

---

## Summary

All 3 gaps from Attempt 1 are now fixed. No new issues found. Design and implementation plan are in complete parity.

---

## Covered (41 requirements → 22 tasks)

- **Gap #1 — Iterate over all content types** → Task 14 (priority-order iteration over all content types)
- **Gap #1 — Extract fields from multipart/form-data** → Task 14 (same `build_fields` path used for all content types)
- **Gap #1 — Extract fields from x-www-form-urlencoded** → Task 14 (covered in priority fallback chain)
- **Gap #1 — Binary fields show as "binary" type** → Task 14 ✓ FIXED: `format_type_display` override for `StringFormat::Binary` → `"binary"`; test assertion corrected from `"string/binary"` to `"binary"`
- **Gap #1 — Store content type on RequestBody** → Task 14 (existing `content_type` field already on struct; plan stores it from the matched key)
- **Gap #2 — ResponseHeader struct (name, type, description)** → Task 2
- **Gap #2 — Extract headers from responses** → Task 9
- **Gap #2 — Text renderer Headers section** → Task 16
- **Gap #2 — JSON renderer headers (passthrough via derive)** → Task 21
- **Gap #3 — Links: ResponseLink model** → Task 3
- **Gap #3 — Links: extraction from responses** → Task 10
- **Gap #3 — Links: drill_command assembled at extraction time** → Task 10 (`build_link_drill_command` helper)
- **Gap #3 — Links: inline display on endpoint detail (name, operationId, params, drill-deeper)** → Task 17
- **Gap #3 — Callbacks: CallbackEntry/Operation/Response models** → Task 4
- **Gap #3 — Callbacks: callbacks field on Endpoint** → Task 4
- **Gap #3 — Callbacks: extraction module** → Task 12
- **Gap #3 — Callbacks: inline extraction on Endpoint** → Task 11
- **Gap #3 — Callbacks: inline text render on parent endpoint** → Task 18
- **Gap #3 — Callbacks: `phyllotaxis callbacks` list subcommand** → Task 22 (Part A text + Part B JSON + Part C CLI)
- **Gap #3 — Callbacks: `phyllotaxis callbacks <name>` detail subcommand** → Task 22 (Part A text + Part B JSON + Part C CLI)
- **Gap #3 — CLI wiring in main.rs** → Task 22 Part C
- **Gap #4 — constraints: Vec<String> on Field** → Task 1
- **Gap #4 — All constraint types extracted (pattern, min/max length, min/max value, multipleOf, exclusiveMin/Max, minItems/maxItems, uniqueItems, minProperties/maxProperties)** → Task 7
- **Gap #4 — Inline display after type** → Task 15 (appended after flags in field render line)
- **Gap #4 — JSON renderer constraints** → Task 20 (`constraints: &f.constraints` in FieldJson)
- **Gap #5 — write_only bool on Field** → Task 1
- **Gap #5 — Extracted from build_fields** → Task 6
- **Gap #5 — Rendered as write-only flag** → Task 15
- **Gap #5 — JSON renderer write_only** → Task 20
- **Gap #5 — Integration test (success criterion #7)** → Task 22 `test_write_only_visible_on_create_user_request` ✓ FIXED
- **Gap #6 — deprecated bool on Field** → Task 1
- **Gap #6 — Extracted from build_fields** → Task 6
- **Gap #6 — Rendered as DEPRECATED flag** → Task 15
- **Gap #6 — JSON renderer deprecated** → Task 20
- **Gap #6 — Integration test (success criterion #8)** → Task 22 `test_deprecated_visible_on_pet_base` ✓ FIXED
- **Gap #7 — title on SchemaModel** → Task 5
- **Gap #7 — Extracted from build_schema_model** → Task 13
- **Gap #7 — Rendered when different from name** → Task 19
- **Gap #7 — JSON renderer title** → Task 20 (SchemaDetailJson extended with title)
- **Gap #8 — Integer enum extraction in build_fields (for Field.enum_values)** → Task 8 (`extract_enum_values` extended)
- **Gap #8 — Integer enum extraction in build_schema_model (for Composition::Enum)** → Task 8 (new `Type::Integer` arm in `build_schema_model`)
- **Gap #8 — Integer enum display** → Task 15 (enums rendered from `enum_values`; no change needed)
- **Success criteria #11 — All existing tests pass** → Task 22 integration test `test_petstore_regression`
- **Verification commands — all 11 success criteria** → Verification Commands section (all criteria mapped to tests)

---

## Gaps Fixed (3 issues from Attempt 1 — all resolved)

### Gap A — Binary field type display discrepancy (Gap #1) — ✅ FIXED

**Design requirement:** "Binary fields (`type: string, format: binary`) display as type 'binary'"

**Issue in Attempt 1:** Task 14's test asserted `file_field.type_display == "string/binary"` — inheriting existing `format_type_display` behavior with no override.

**Fix applied:** Task 14 now specifies (lines 1264-1279):
- Special case in `format_type_display` within the `String` arm
- Check `if matches!(fmt, openapiv3::StringFormat::Binary)` before the format match
- Return `"binary".to_string()` directly (not `"string/binary"`)
- Test assertion corrected to assert `"binary"`

**Verification:** Implementation plan includes both the code location and test assertion.

---

### Gap B — Multipart per-field encoding info not explicitly descoped (Gap #1) — ✅ FIXED

**Design requirement:** "For multipart, extract encoding info (contentType overrides per field)"

**Issue in Attempt 1:** Task 14 had only `// TODO:` comment deferring this decision.

**Fix applied:** Task 14 now includes explicit scope decision (lines 1281-1293):
- Feature is **intentionally descoped** (not deferred)
- Clear justification: encoding describes wire transmission format, not the field's logical schema type
- Field type is unchanged regardless of encoding override
- For phyllotaxis's goal, schema-level info is sufficient
- Future iteration path documented: add `encoding: Option<String>` to `Field`
- Extraction site marked with NOTE comment (not TODO)

**Verification:** Implementation plan includes both the scope decision text and the exact NOTE comment to leave in code.

---

### Gap C — Integration tests missing for success criteria #7 and #8 (Gaps #5 and #6) — ✅ FIXED

**Design criteria:**
- #7: `CreateUserRequest` — password shows `[write-only]`
- #8: `PetBase` — legacy_code shows `[DEPRECATED]`

**Issue in Attempt 1:** Only unit tests covered these. No integration tests.

**Fix applied:** Task 22's integration test block now includes (lines 2303-2316):
- `test_write_only_visible_on_create_user_request` — runs `schemas CreateUserRequest` against kitchen-sink, asserts `"write-only"` present
- `test_deprecated_visible_on_pet_base` — runs `schemas PetBase` against kitchen-sink, asserts `"DEPRECATED"` present

Verification Commands section now includes (lines 2347-2348):
- `cargo test --test integration_tests test_write_only_visible_on_create_user_request    # criterion 7`
- `cargo test --test integration_tests test_deprecated_visible_on_pet_base               # criterion 8`

**Verification:** Both tests are present in the integration test block with proper assertions and criterion numbering.

---

## Final Sweep Results

### TODO/FIXME audit
✅ No unfinished TODO, FIXME, or XXX comments found in implementation plan. Gap B's scope decision is documented with NOTE comment (not TODO).

### Integration test coverage
✅ All 11 success criteria have corresponding integration tests:
1. test_multipart_body_visible_in_upload_endpoint
2. test_response_headers_visible
3. test_callbacks_list_kitchen_sink
4. test_callbacks_detail_on_event
5. test_links_visible_on_post_users
6. test_schema_constraints_visible
7. test_write_only_visible_on_create_user_request
8. test_deprecated_visible_on_pet_base
9. test_schema_title_visible
10. test_integer_enum_visible
11. test_petstore_regression

### Design↔Plan alignment
✅ Complete parity. All design success criteria (lines 144-158 of design.md) map to integration tests with same scope.

### Task definitions
✅ All 22 tasks have clear scope, test assertions, and verification locations.

### Verification commands
✅ Complete set provided (lines 2329-2358 of implementation.md):
- `cargo test --lib` (all unit tests + regressions)
- `cargo test --test integration_tests` (all integration tests)
- Individual criterion tests (11 specific tests)
- Manual smoke tests (5 commands against kitchen-sink fixture)

---

## Conclusion

✅ **PASSED — Design and implementation plan are in complete parity.**

All 3 gaps from Attempt 1 are fixed:
- Gap A: Binary type display — explicit code location and test assertion
- Gap B: Encoding scope — explicit scope decision with justification, not TODO
- Gap C: Integration tests for criteria #7 and #8 — both tests present with assertions

No new issues detected. Plan is ready for Beads creation.
