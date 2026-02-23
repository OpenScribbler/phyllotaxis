# Phyllotaxis — Design vs. Implementation Plan Validation Report

**Date:** 2026-02-20
**Design doc:** `docs/plans/2026-02-20-phyllotaxis-design.md`
**Implementation plan:** `docs/plans/2026-02-20-phyllotaxis-implementation.md`
**Validator:** Formal parity check (Claude)

---

## Section 1 — Feature Coverage

### 1.1 Commands

**All 8 commands / command modes:**

- ✅ **Overview (L0):** Epic 4 (Tasks 4.1–4.4) implements the overview model, plain text render, JSON render, and wires it into main as the `None` subcommand branch.
- ✅ **Resources L1:** Epic 5 (Tasks 5.1–5.4) implements `extract_resource_groups` and the listing render.
- ✅ **Resources L2:** Epic 6 (Tasks 6.1–6.5) implements group lookup, detail model, and both renders.
- ✅ **Resources L3:** Epic 7 (Tasks 7.1–7.6) implements full endpoint detail model, both renders, and wires all three arguments (`name`, `method`, `path`).
- ✅ **Schemas list:** Task 8.1 implements `list_schemas`, Task 8.4 renders it, Task 8.7 wires the `name: None` branch.
- ✅ **Schemas detail:** Tasks 8.2, 8.5, 8.6, 8.7 implement the model, both renders, and wire the `name: Some(name)` branch.
- ✅ **Auth:** Epic 9 (Tasks 9.1–9.3) implements model, plain text render, JSON render, and wires `Commands::Auth`.
- ✅ **Search:** Epic 10 (Tasks 10.1–10.3) implements the search function, both renders, and wires `Commands::Search`.
- ✅ **Init:** Epic 11 (Tasks 11.1–11.2) implements framework detection, spec candidate search, interactive flow, and wires `Commands::Init`.

### 1.2 Global Flags

- ✅ **--spec:** Defined in Task 1.2 as `spec: Option<PathBuf>` on `Cli`. Used in `load_spec` in Task 2.4. Audit confirmed in Task 12.2.
- ✅ **--json:** Defined in Task 1.2 as `json: bool` on `Cli`. Every command render branches on `cli.json` (confirmed in Task 12.2).
- ✅ **--expand:** Defined in Task 1.2 as `expand: bool` on `Cli`. Task 8.7 checks `--expand` before calling `expand_schema`.

### 1.3 Config File with Variable Overrides

- ✅ **Config struct:** Task 2.1 defines `Config` with `spec: Option<String>` and `variables: Option<HashMap<String, String>>`.
- ✅ **Config discovery:** Task 2.1 implements `load_config` walking up from `start_dir`, stopping at root.
- ✅ **Variable resolution:** Task 4.1 uses `config.variables` to resolve `{var}` placeholders in server URLs via string replacement.

### 1.4 Deprecation Markers [DEPRECATED]

- ✅ **Detection:** Task 3.5 implements `is_deprecated_tag` checking tag names for `(Deprecated)` and extensions for `x-deprecated: true`.
- ✅ **L1 listing:** Task 5.2 renders `[DEPRECATED]` marker after slug in the resource list.
- ✅ **L2 endpoint listing:** Task 6.3 renders `[DEPRECATED]` after summary for deprecated endpoints.
- ✅ **Fixture coverage:** Task 1.4 includes a deprecated endpoint (`DELETE /pets/{id}`) and a deprecated tag group (`Deprecated Pets`).
- ✅ **Integration test:** Task 14.2 asserts `[DEPRECATED]` marker in resource list output.

### 1.5 Alpha Markers [ALPHA]

- ✅ **Detection:** Task 3.5 implements `is_alpha_tag` checking tag names for `(Alpha)` and extensions for `x-alpha: true`.
- ✅ **L1 listing:** Task 5.2 renders `[ALPHA]` marker in the resource list.
- ✅ **L2 endpoint listing:** Task 6.3 renders `[ALPHA]` for alpha endpoints.
- ⚠️ **Fixture gap:** The petstore fixture in Task 1.4 includes a deprecated tag (`Deprecated Pets`) but no alpha tag or alpha endpoint. `is_alpha` detection will have no fixture-based integration test coverage. The unit tests in Task 3.5 test `is_alpha_tag` in isolation, but no integration test exercises the `[ALPHA]` render path end-to-end.

### 1.6 Schema Composition

- ✅ **allOf flattening:** Task 7.3 extends `build_fields` to merge all `allOf` referenced schemas into a flat list. Task 8.2 detects `allOf` composition and calls this path.
- ✅ **oneOf/anyOf labeling:** Task 8.2 detects `oneOf` and `anyOf`, sets `Composition::OneOf(variant_names)` / `Composition::AnyOf(variant_names)`, and leaves `fields = []`.
- ✅ **Render:** Task 8.5 renders the composition block with "One of:" / "Any of:" and schema links, or notes that allOf fields are merged.
- ⚠️ **Fixture gap:** The petstore fixture covers `allOf` (via `PetList`) but has no `oneOf` or `anyOf` schema. The `oneOf`/`anyOf` render path has no integration test coverage.

### 1.7 --expand with Depth Limit (5) + Cycle Detection

- ✅ **Depth limit:** Task 8.3 implements `expand_field` with `depth: usize`, stopping at `depth == 0` and marking `[max depth]`.
- ✅ **Default depth:** Task 8.3 calls `expand_field` with `depth = 5` from `expand_schema`.
- ✅ **Cycle detection:** Task 8.3 uses a `visited: &mut Vec<String>` stack, marking circular fields as `[circular: {SchemaName}]`.
- ✅ **Unit test:** `test_cycle_detection` explicitly covers the circular case.
- ✅ **Integration test:** Task 14.3 `test_schema_detail_expanded` verifies the expanded output for `Pet`.

### 1.8 Field Format Info (string/uuid, string/date-time)

- ✅ **type_display construction:** Task 7.2 sets `type_display` as `"string/uuid"`, `"string/date-time"` when format is present.
- ✅ **Render:** Task 7.4 includes `type_display` in the field row.
- ✅ **Unit test:** `test_build_fields_pet` asserts `id` has type `string/uuid`.
- ✅ **Integration test:** Task 14.3 `test_schema_detail_pet` asserts `string/uuid` appears in output.

### 1.9 Nullable Marking

- ✅ **Model:** Task 3.2 includes `nullable: bool` on `Field`.
- ✅ **Population:** Task 7.2 sets `nullable` from schema properties.
- ✅ **Render:** Task 7.4 includes `nullable` in modifiers (e.g., `(optional, nullable)`).
- ✅ **Unit test:** `test_build_fields_pet` asserts `nickname` is nullable.

### 1.10 Server URL Template Handling + Variable Resolution

- ✅ **Template display:** Task 4.1 keeps the template URL (e.g., `https://{tenant}.example.com`) when no config variables are present.
- ✅ **Variable resolution:** Task 4.1 does `{var}` string replacement using `config.variables`.
- ✅ **ServerVar collection:** Task 4.1 collects `ServerVar` structs (name, required, description, default) from `server.variables`.
- ✅ **Render:** Task 4.2 renders "Variables:" block under Base URL.
- ✅ **Fixture:** Task 1.4 includes a server with a `{env}` template variable.

### 1.11 Enum Values Inline

- ✅ **Model:** Task 3.2 includes `enum_values: Vec<String>` on `Field`. Task 3.1 includes the same on `Parameter`.
- ✅ **Population:** Task 7.2 sets `enum_values` from schema's `enum` field.
- ✅ **Render:** Task 7.4 appends `Enum: [active, inactive, sold]` inline after description.
- ✅ **Integration test:** Task 14.3 `test_schema_detail_pet` asserts `Enum:` appears in output.

### 1.12 Short Examples Inline, Body Examples Separate

- ✅ **Request body example:** Task 7.1 extracts `example` from the media type object into `RequestBody.example`.
- ✅ **Response example:** Task 7.1 extracts the first `application/json` example into `Response.example`.
- ✅ **Render — Request Example separate section:** Task 7.4 renders "Request Example:" as a separate section below field rows.
- ✅ **Render — Response Example separate section:** Task 7.4 renders "Response Example:" as a separate section.
- ⚠️ **"Short examples inline" gap:** The design doc (Key Decision #10) specifies "Short examples inline" for individual fields, distinct from body examples. The plan's `Field` struct (Task 3.2) has no `example` field for per-field inline examples. Task 7.2 (`build_fields`) does not extract per-field examples from property-level `example` nodes in the spec. The plan only captures body-level examples, not field-level ones. This partially misses the design's intent — though the design's example output doesn't actually show any field-level inline examples, so the "short examples inline" clause is ambiguous. The implementation covers body examples fully; field-level examples are unimplemented.

### 1.13 Documentation Surfacing

- ✅ **info.description at L0:** Task 4.1 truncates `api.info.description` to 200 chars for the overview.
- ✅ **Tag description at L2:** Task 3.1 `ResourceGroup.description` is populated from tag description (Task 5.1). Task 6.3 renders it below the resource name.
- ✅ **Operation summary at L1/L2:** Task 5.1 populates `Endpoint.summary`. Task 5.2 renders summaries in the listing. Task 6.3 renders summaries in the endpoint table.
- ✅ **Operation description at L3:** Task 3.1 `Endpoint.description` is populated. Task 7.4 renders it below the endpoint path, truncated at 500 chars.
- ✅ **Schema description:** Task 8.2 sets `description`, truncated to 500 chars. Task 8.5 renders it below the schema name.
- ✅ **Property description:** Task 3.2 `Field.description` is populated (Task 7.2). Task 7.4 renders it inline.
- ✅ **externalDocs:** Task 3.1 `Endpoint.external_docs`. Task 3.3 `SchemaModel.external_docs`. Task 8.5 renders "See also:" links.
- ✅ **Missing descriptions omitted:** Task 4.2 and Task 7.4 both explicitly skip absent descriptions (no "No description" placeholders).

### 1.14 Search (Case-Insensitive Substring, Grouped by Type)

- ✅ **Case-insensitive:** Task 10.1 lowercases `term` before comparison.
- ✅ **Substring matching:** Task 10.1 checks resource slugs/descriptions, endpoint paths/summaries/descriptions, and schema names for substring presence.
- ✅ **Grouped by type:** Task 10.1 returns `SearchResults` with separate `resources`, `endpoints`, `schemas` fields in fixed order.
- ✅ **Render:** Task 10.2 renders each section under its own header, omitting empty sections.
- ✅ **Drill-deeper hints:** Task 10.2 renders `phyllotaxis resources <slug>` and `phyllotaxis schemas <name>` hints.

### 1.15 Resource Name Slugification

- ✅ **Lowercase + hyphens:** Task 3.4 `slugify` lowercases and replaces spaces with hyphens.
- ✅ **PascalCase splitting:** Task 3.4 inserts hyphens before uppercase letters following lowercase (`DiscoveryIntegration` → `discovery-integration`).
- ✅ **Deprecated/Alpha suffix stripping:** Task 3.4 strips `(Deprecated)`, `(deprecated)`, `(Alpha)`, `(alpha)` before processing.
- ✅ **Unit tests:** Four unit tests cover spaces, PascalCase, deprecated suffix, and alpha suffix.

### 1.16 Spec Discovery Priority

- ✅ **Flag > config > auto-detect > error:** Task 2.2 implements `resolve_spec_path` with exactly this priority order. Task 2.1 provides config loading, Task 2.2 step 3 implements auto-detection (search 2 levels for `openapi:` content).
- ✅ **Error message explains priority:** Task 2.2 specifies the `Err(...)` message should explain the resolution order.
- ✅ **Unit tests:** `test_resolve_prefers_flag` and `test_resolve_autodetect` cover the first and third cases.
- ⚠️ **Config path resolution detail:** Task 2.2 specifies "resolve relative to the config file's directory" for config-sourced paths, but the implementation of `load_config` (Task 2.1) returns a `Config` struct that doesn't carry the directory it was found in. The caller in `resolve_spec_path` would need to know the config file's directory to resolve relative paths from it. This is an implicit dependency not made explicit in Task 2.2 — the function signature doesn't receive the config file's location, only the `Config` struct itself. This could cause a bug where a config-specified relative path is resolved from cwd instead of from the config file's directory.

---

## Section 2 — Decision Alignment

Checking all 17 Key Decisions from the design doc's table:

| # | Decision | Chosen Approach | Plan Alignment |
|---|----------|-----------------|----------------|
| 1 | Parser crate | `openapiv3` v2.2.0 | ✅ Task 1.1 specifies `openapiv3 = "2.2.0"` exactly. |
| 2 | `auth` command output | Mirror spec's security schemes | ✅ Task 9.1 extracts scheme type, detail, and usage count. Task 9.2 renders per the design example. |
| 3 | `--json` output | Structured progressive disclosure (not raw spec) | ✅ All JSON renderers serialize the curated model structs, not the raw openapiv3 types. Task 12.1 audits for this. |
| 4 | Level 3 parameters | Show all types (path, query, header) grouped | ✅ Task 3.1 defines `ParameterLocation` enum with `Path`, `Query`, `Header`. Task 7.1 merges path-level and operation-level parameters. Task 7.4 renders them in separate sections. |
| 5 | `--expand` safety | Depth limit (5) + cycle detection | ✅ Task 8.3 uses `depth = 5` and `visited` stack with `[circular: SchemaName]` marking. |
| 6 | `init` command | In POC scope | ✅ Epic 11 is fully included in the plan with framework detection and interactive setup. |
| 7 | Deprecation markers | `[DEPRECATED]` only, no replacement hint | ✅ Task 5.2 and 6.3 render only `[DEPRECATED]` — no replacement hint logic anywhere in the plan. |
| 8 | Alpha/beta markers | `[ALPHA]` same pattern as deprecated | ✅ Tasks 5.2 and 6.3 treat `[ALPHA]` identically to `[DEPRECATED]` in listing rendering. |
| 9 | Server URL variables | Show template + details, resolve with config | ✅ Task 4.1 shows template when no config variables, resolves with config when present. `ServerVar` collection includes required/optional flag and description. |
| 10 | Enums and examples | Inline enums, inline short examples, separate body examples | ⚠️ Enum values are correctly inline (Task 7.4). Body examples are correctly separate. However, per-field inline examples are not implemented (no `example` on `Field` struct, not extracted in `build_fields`). See gap 1.12. |
| 11 | Spec parsing strategy | Stateless — parse every invocation | ✅ No caching, no daemon. Every `main()` invocation calls `load_spec`. |
| 12 | Search ranking | Flat list grouped by type (Resources → Endpoints → Schemas) | ✅ Task 10.1 `SearchResults` struct has fixed-order fields. No ranking within groups. |
| 13 | Schema composition | Flatten `allOf`, label `oneOf`/`anyOf` | ✅ Task 7.3 flattens allOf. Task 8.2 labels oneOf/anyOf with variant names. Task 8.5 renders both correctly. |
| 14 | Level 3 response body | Reference + example (not inline fields) | ✅ Task 7.1 populates `Response.schema_ref` with the schema name (for "Returns: X") rather than inlining all fields. Task 7.4 renders "Returns: SchemaName" and a separate "Response Example:" section. |
| 15 | Field format info | Show as type/format | ✅ Task 7.2 builds `type_display` as `"string/uuid"`, `"string/date-time"` when format is present. |
| 16 | Nullable marking | Show alongside optional | ✅ Task 3.2 `Field.nullable`. Task 7.4 renders `(optional, nullable)` combined modifier. |
| 17 | Spec documentation | Summary in listings, full in detail | ✅ Task 5.1/5.2 use `summary` in L1/L2. Task 7.1/7.4 use full `description` in L3 (truncated at 500 chars). Missing descriptions omitted. `externalDocs` as "See also:" links. |

**Decision alignment result:** 16 of 17 decisions fully aligned. Decision #10 is partially aligned (gap identified in feature coverage section 1.12).

---

## Section 3 — Output Format Parity

Comparing design doc example outputs against plan renderers:

### L0 Overview
Design example:
```
API: Aembit Cloud API
Base URL: https://{tenant}.aembit-eng.com
  Variables:
    tenant  (required)  Your Aembit tenant name
Auth: Bearer token (HTTP bearer)

Commands:
  phyllotaxis resources    List all resource groups (31 available)
  phyllotaxis schemas      List all data models (143 available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

Task 4.2 render template:
```
API: {title}
{description (if present)}
Base URL: {url}
  Variables:
    {name}  ({required/optional})  {description}
Auth: {scheme display}

Commands:
  phyllotaxis resources    List all resource groups ({N} available)
  ...
```

✅ Format matches. Description placement (before Base URL) is consistent. Variables indentation with 2+4 spaces matches. Auth line format matches. Commands section matches.

### L1 Resource Listing
Design:
```
Resources:
  access-condition           Aembit Access Conditions for policy evaluation
  access-policy              [DEPRECATED] Access policies
  discovery-integration      [ALPHA] Discovery integration endpoints
```

Task 5.2: column-aligned slug, marker, description. `[DEPRECATED]` and `[ALPHA]` after slug.

✅ Format matches. Column alignment rule is specified.

### L2 Resource Detail
Design:
```
Resource: Access Policies
Description: Define which client workloads...

Endpoints:
  GET    /access-policies           List all access policies
  DELETE /access-policies/{id}      Delete an access policy

Drill deeper:
  phyllotaxis resources access-policies GET /access-policies
```

Task 6.3: renders `Resource: {display_name}`, then description, then endpoints table with column-aligned METHOD and path.

⚠️ **Format discrepancy:** The design example shows "Description:" as a labeled line (`Description: Define which client workloads...`). Task 6.3's render template omits the "Description:" label and just shows the description text directly below the resource name. This is a minor but real format difference. The plan should either add the label or the design should be followed as written.

### L3 Endpoint Detail
Design:
```
POST /access-policies
Create a new access policy.

Authentication: Bearer token (required)

Path Parameters:
  (none)

Query Parameters:
  (none)

Request Body (application/json):
  name              string   (required)  Display name for the policy
```

Task 7.4 template matches this structure closely.

✅ Format matches. `(none)` for empty sections, indented field rows, separate sections for examples, errors, and drill-deeper.

### Schema Detail
Design:
```
Schema: AccessPolicyV2DTO

Fields:
  id                  string/uuid             (read-only)  Unique identifier
  status              string                               Enum: [active, inactive, pending]
  nickname            string                  (optional, nullable)  Can be cleared with null

Related schemas:
  phyllotaxis schemas EntityMetaDTO
```

Task 8.5 uses the same field row format (same columns as Level 3 request body fields) and "Related schemas:" section.

✅ Format matches.

### Schema Detail — Expanded
Design:
```
Schema: AccessPolicyV2DTO (expanded)

Fields:
  clientWorkload      EntityMetaDTO:
    id                  string                (read-only)  Unique identifier
    name                string                             Display name
```

Task 8.5: renders nested fields indented under the parent with 2 extra spaces.

✅ Format matches.

### Auth Command
Design:
```
Authentication:
  bearer (HTTP)
    Scheme: bearer
    Description: JWT token for API access

  Used by: 142 operations (all endpoints)

Drill deeper:
  phyllotaxis resources    Browse endpoints by resource group
```

Task 9.2 matches this structure, including the "(all endpoints)" qualifier when usage_count equals total operation count.

✅ Format matches.

### Search
Design:
```
Results for "workload":

Resources:
  client-workload              Workloads that initiate access requests

Endpoints:
  GET  /api/v1/client-workloads              List all client workloads

Schemas:
  ClientWorkloadExternalDTO

Drill deeper:
  phyllotaxis resources client-workload
  phyllotaxis schemas ClientWorkloadExternalDTO
```

Task 10.2 matches this structure. Empty sections omitted.

✅ Format matches.

---

## Section 4 — Architecture Alignment

Design doc module structure:
```
src/
  main.rs
  spec.rs
  commands/
    overview.rs
    resources.rs
    schemas.rs
    auth.rs
    search.rs
    init.rs
  render/
    text.rs
    json.rs
  models/
    resource.rs
    schema.rs
```

Plan module structure (Task 1.3):
```
src/
  main.rs
  spec.rs
  models/
    mod.rs
    resource.rs
    schema.rs
  commands/
    mod.rs
    overview.rs
    resources.rs
    schemas.rs
    auth.rs
    search.rs
    init.rs
  render/
    mod.rs
    text.rs
    json.rs
```

✅ **Architecture matches.** The plan adds `mod.rs` files for each module directory — a necessary Rust-specific implementation detail not mentioned in the design doc (which is language-agnostic in its module listing). This is an expected and correct addition, not a deviation.

✅ All command modules are present. No extra modules added, none removed.

✅ The plan correctly places `slugify`, `is_deprecated_tag`, and `is_alpha_tag` in `models/resource.rs` and `resolve_schema` in `spec.rs`, consistent with the design's separation of concerns.

✅ No unexplained deviations in architecture.

---

## Section 5 — Missing Dependencies

Examining implicit dependencies that lack explicit declarations:

1. ✅ **Task 7.1 → Task 7.2:** Task 7.1 says "Keep it focused: `$ref` schema resolution uses `resolve_schema` from `spec.rs`." It lists "Task 3.2, Task 3.6, Task 6.2" as dependencies. Task 7.2 (`build_fields`) is called by Task 7.1 but Task 7.2 is not listed as a dependency of Task 7.1 — however, Task 7.1 says only to find the operation and populate the struct; it delegates field building to 7.2. Task 7.4 depends on Task 7.1. The plan has: 7.4 depends on 7.1, 7.1 depends on 3.2/3.6/6.2, and 7.2 depends on 3.6. But Task 7.1's description says "Populates `RequestBody`: finds the `application/json` content type, resolves the schema, and builds a flat `Vec<Field>` from its properties" — this implies calling `build_fields` from 7.2. Task 7.1 is declared as depending on Task 3.2 and 3.6 only, not on Task 7.2. **Task 7.1 should declare Task 7.2 as a dependency** since it calls `build_fields`.

2. ✅ **Task 8.2 → Task 7.3:** Task 8.2 calls `build_fields` with allOf flattening, which is implemented in Task 7.3. Task 8.2 correctly lists "Task 7.3, Task 8.1" as dependencies.

3. ⚠️ **Task 4.1 → Task 3.4 and 3.5 (slug/status for resource counts):** Task 4.1 counts "unique tags (resource_count)" from the spec. It doesn't need `slugify` or status detection — it just counts tag entries. No missing dependency here.

4. ⚠️ **Task 5.1 → Task 3.6 (ref resolution for parameters):** Task 5.1 populates endpoints with `method`, `path`, `summary`, `is_deprecated`, and `is_alpha` — it does not resolve parameter `$ref`s at Level 1. That happens in Task 7.1. So Task 5.1 does not need Task 3.6 as a dependency. The dependency table at the bottom confirms "Epic 5 can start after Epics 2, 3" — this is correct.

5. ⚠️ **Task 2.2's config-path tracking gap (re: Gap 3 identified in Section 1.16):** Task 2.2 declares "Requires Task 2.1" as its dependency, which is correct. However, the implicit information dependency — that `load_config` must return the *path* of the found config file, not just the parsed `Config`, for relative spec path resolution to work — is unaddressed. This is a functional gap, not just a missing task dependency declaration.

6. ✅ **Task 9.3 wires main:** Task 9.3 explicitly says "In `main.rs`, wire `Commands::Auth`" and depends on Task 9.2. Correct.

7. ✅ **Task 10.3 wires main:** Task 10.3 explicitly says "In `main.rs`, wire `Commands::Search { term }`" and depends on Task 10.2. Correct.

8. ✅ **Task 11.2 wires main:** Task 11.2 explicitly says "Wire `Commands::Init` in `main.rs`." Correct.

9. ✅ **Epic 8 → Epic 7.3:** The dependency table lists "Epic 8 (Schemas) can start after Epics 2, 3, 7.3" — correctly capturing that the allOf flattening in Task 7.3 must exist before Task 8.2 can call it.

10. ✅ **Task 14.1 depends on Task 4.4:** Integration tests depend on wiring tasks, not just logic tasks. All four integration test tasks list their wiring tasks as dependencies. Correct.

---

## Gap Summary

The following issues were identified:

1. **Gap 1 — [ALPHA] marker has no integration test coverage** (Section 1.5): The petstore fixture has no alpha tag or endpoint. The `[ALPHA]` render path is exercised by unit tests only. An alpha tag and at least one alpha-tagged endpoint should be added to the fixture (Task 1.4) and a corresponding integration test added (Task 14.2) to validate the end-to-end path.

2. **Gap 2 — oneOf/anyOf render path has no integration test coverage** (Section 1.6): The petstore fixture has no `oneOf` or `anyOf` schema. At minimum, a schema using `oneOf` should be added to the fixture (Task 1.4) and a `test_schema_oneof` integration test added (Task 14.3).

3. **Gap 3 — Per-field inline examples not implemented** (Section 1.12 / Decision #10): The design specifies "inline short examples" as distinct from body examples. The `Field` struct has no `example` field, and `build_fields` does not extract per-property `example` nodes from the spec. The design's example outputs don't demonstrate this clearly (no field-level examples are shown in the example blocks), so the impact is limited — but the design doc explicitly calls it out in Key Decision #10 and in the POC Scope list ("Short examples inline, body examples separate"). This should either be implemented or explicitly descoped.

4. **Gap 4 — L2 resource detail "Description:" label discrepancy** (Section 3, L2 output): The design shows `Description: Define which client workloads...` as a labeled line. Task 6.3's render template shows the description text without the "Description:" label. This is a minor but verifiable format deviation. The implementation should match the design exactly.

5. **Gap 5 — Config-relative spec path resolution loses the config file location** (Section 1.16 / Section 5, item 5): `load_config` returns `Option<Config>` but not the `PathBuf` of where the config file was found. `resolve_spec_path` receives `&Option<Config>` and would resolve `config.spec` relative to the process's cwd rather than the config file's directory. This is a functional correctness bug. `load_config` should return `Option<(Config, PathBuf)>` (or equivalent), and `resolve_spec_path` should use that path for resolution.

6. **Gap 6 — Task 7.1 missing dependency on Task 7.2** (Section 5, item 1): Task 7.1's description includes building `RequestBody.fields` using `build_fields`, which is implemented in Task 7.2. The declared dependencies for Task 7.1 are "Task 3.2, Task 3.6, Task 6.2" — Task 7.2 is missing. This means a developer following the plan sequentially could write Task 7.1 before Task 7.2 exists.

---

## VALIDATION FAILED — 6 gaps found

1. **[ALPHA] marker has no integration test coverage** — Add an alpha tag/endpoint to the petstore fixture and an integration test asserting `[ALPHA]` appears in output.
2. **oneOf/anyOf has no integration test coverage** — Add a `oneOf` schema to the petstore fixture and a `test_schema_oneof` integration test.
3. **Per-field inline examples not implemented** — Either add `example: Option<serde_json::Value>` to `Field` and extract it in `build_fields`, or explicitly add "per-field inline examples" to the Out of Scope list in the design doc.
4. **L2 "Description:" label missing from render** — Task 6.3 should render `Description: {text}` (with label), matching the design doc's example output.
5. **Config-relative spec path loses config file location** — `load_config` must return the config file's `PathBuf` alongside the parsed `Config` so that relative `spec:` paths can be resolved correctly from the config file's directory, not from cwd.
6. **Task 7.1 missing dependency on Task 7.2** — Add `Task 7.2` to Task 7.1's dependency declaration, since `get_endpoint_detail` calls `build_fields` which is defined in Task 7.2.
