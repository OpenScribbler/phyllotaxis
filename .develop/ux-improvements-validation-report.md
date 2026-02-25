## Validation Report

**Feature:** ux-improvements
**Design doc:** `docs/plans/2026-02-24-ux-improvements-design.md`
**Implementation plan:** `docs/plans/2026-02-24-ux-improvements-implementation.md`
**Date:** 2026-02-24
**Attempt:** 1/5

---

### ✅ Covered (9 requirements → 6 tasks)

- Change 1: Search indexes schema field names → Task 1
- Change 2: `--expand` inlines array-of-ref fields → Task 2
- Change 3: Search results show match reason → Task 3
- Change 4: Suppress empty parameter sections → Task 4
- Change 5a: Auth view drill-deeper hints → Task 5 (implemented as `phyllotaxis search <scheme_name>` — plan documents deviation from literal design; `AuthModel` lacks resource slug data needed for `phyllotaxis resources <name>` as written)
- Change 5b: Schema listing drill-deeper hint → Task 5 (ADDED by this validation pass — was missing)
- Change 6: Search result counts → Task 5
- Change 7: NonAdminRole base type display → Task 6
- Change 8: Field alignment fix → Task 4
- `SchemaMatch.matched_field: Option<String>` → Task 1 (Step 2, struct definition)
- `EndpointMatch.matched_on: Option<String>` → Task 3 (Step 2, struct definition)
- `SchemaModel.base_type: Option<String>` → Task 6 (Step 2, model field)
- Array-of-ref expand uses existing depth cap of 5 → Task 2 (explicitly stated in Why this approach)
- Text rendering changes don't affect JSON output (except where specified) → Confirmed: Changes 4 and 8 are text-only; JSON gains new fields only for Changes 1, 3, 5 (counts), 6, 7 via serde derive
- All tests use kitchen-sink fixture → Confirmed in all test code snippets across Tasks 1–6

---

### ❌ Gaps Found (1 issue)

1. **Missing Coverage:** Change 5 (Consistent drill-deeper hints) specifies two sub-items:
   - Schema listing → `phyllotaxis schemas <name>` (MISSING from plan)
   - Auth view → per-scheme hints (covered in Task 5)

   The plan's Task 5 covered only the auth view drill-deeper hint. The schema listing
   drill-deeper hint had no test, no implementation step, and no commit coverage.

---

### Fix Applied

The gap was fixed in the implementation plan:

1. Added `test_schema_listing_drill_deeper_hint` unit test (in `text.rs` module tests)
   and `test_schema_listing_shows_drill_deeper_hint` integration test to Task 5 Step 1.

2. Added **Step 4 — Implement: schema listing drill-deeper hint** to Task 5, specifying:
   - Location: `render_schema_list` in `text.rs`
   - Implementation: TTY-gated `Drill deeper:` block with `phyllotaxis schemas <name>` hint
   - Callout: verify whether `render_schema_list` already accepts `is_tty: bool`

3. Renumbered Step 4 (Verify) → Step 5, Step 5 (Commit) → Step 6.

4. Updated the commit message to reflect both schema listing and auth changes.

---

### ✅ PASSED

One gap found and fixed. All 8 design changes now have full task coverage. All 3 data
model additions (`matched_field`, `matched_on`, `base_type`) are explicitly specified.
Depth cap, JSON compatibility, and fixture usage all confirmed. Plan is ready for
implementation.
