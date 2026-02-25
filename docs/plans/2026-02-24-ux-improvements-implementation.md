# UX Improvements - Implementation Plan

**Feature:** ux-improvements
**Design doc:** `docs/plans/2026-02-24-ux-improvements-design.md`
**Date:** 2026-02-24

---

## Overview

Eight targeted changes across five source files. Grouped into six tasks — two meaty
changes (search field indexing, expand array-ref) get their own tasks; the four lighter
changes are combined by logical locality (text.rs rendering changes together, JSON
additions together).

**Baseline:** 200 tests pass before any changes (`cargo test`).

---

## Task grouping

| Task | Changes | Files touched |
|------|---------|---------------|
| Task 1 | Search indexes field names (#1) | `search.rs` |
| Task 2 | Expand inlines array-of-ref (#2) | `schemas.rs` |
| Task 3 | Search match reason (#3) | `search.rs`, `text.rs`, `json.rs` |
| Task 4 | Suppress empty param sections + field alignment fix (#4, #8) | `text.rs` |
| Task 5 | Search result counts + consistent drill-deeper hints (#5, #6) | `text.rs`, `json.rs` |
| Task 6 | NonAdminRole base type display (#7) | `models/schema.rs`, `schemas.rs`, `text.rs`, `json.rs` |

---

## Task 1: Search indexes schema field names

**Design change #1** — `search.rs` only.

### Why this approach

The search function currently filters schemas purely by name. Extending it to also check
field names requires iterating the schema's properties. The `find_schema` + `build_fields`
pipeline already exists in `schemas.rs`; we call `find_schema` here and walk the
`SchemaKind::Type::Object` properties manually (without building full `Field` structs,
since we only need the field name strings). A `matched_field: Option<String>` on
`SchemaMatch` lets the renderers annotate the result without changing the core data
contract.

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs`

### Step 1 — Write the failing test

Add to the `#[cfg(test)] mod tests` block at the bottom of `search.rs`:

```rust
fn load_kitchen_sink_api() -> openapiv3::OpenAPI {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
    serde_yaml_ng::from_str(&content).unwrap()
}

#[test]
fn test_search_field_name_email() {
    let api = load_kitchen_sink_api();
    let results = search(&api, "email");

    // Should find schemas that have an "email" field
    assert!(
        !results.schemas.is_empty(),
        "Search for 'email' should find schemas with email fields"
    );
    let names: Vec<&str> = results.schemas.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"User"),
        "Expected User (has email field), got: {:?}",
        names
    );
    assert!(
        names.contains(&"CreateUserRequest"),
        "Expected CreateUserRequest (has email field), got: {:?}",
        names
    );

    // Matches via field should have matched_field populated
    let user_match = results.schemas.iter().find(|s| s.name == "User").unwrap();
    assert_eq!(
        user_match.matched_field.as_deref(),
        Some("email"),
        "User match should annotate matched_field='email'"
    );
}

#[test]
fn test_search_field_name_does_not_shadow_name_match() {
    let api = load_kitchen_sink_api();
    // "User" matches by name — matched_field should be None
    let results = search(&api, "user");
    let user_match = results.schemas.iter().find(|s| s.name == "User");
    assert!(user_match.is_some(), "User should still match by name");
    assert!(
        user_match.unwrap().matched_field.is_none(),
        "Name-matched schema should not have matched_field set"
    );
}
```

Run: `cargo test -p phyllotaxis search_field_name`
Expected: **FAIL** (field `matched_field` does not exist on `SchemaMatch`)

### Step 2 — Implement

Replace the `SchemaMatch` struct and the schema search block in `search.rs`:

```rust
// Old:
#[derive(Debug, serde::Serialize)]
pub struct SchemaMatch {
    pub name: String,
}

// New:
#[derive(Debug, serde::Serialize)]
pub struct SchemaMatch {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_field: Option<String>,
}
```

Replace the schema search section (currently lines ~133–138 of `search.rs`):

```rust
// Search schemas — by name OR by field name
let mut schemas: Vec<SchemaMatch> = Vec::new();
for name in list_schemas(api) {
    if name.to_lowercase().contains(&term_lower) {
        // Name match: no field annotation
        schemas.push(SchemaMatch { name, matched_field: None });
        continue;
    }

    // Field name match: look inside the schema's properties
    if let Some(schema) = crate::commands::schemas::find_schema(api, &name) {
        let field_match = match &schema.schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
                obj.properties.keys().find(|k| k.to_lowercase().contains(&term_lower)).cloned()
            }
            openapiv3::SchemaKind::AllOf { all_of } => {
                // Walk allOf subschemas for inline object properties
                all_of.iter().find_map(|sub| {
                    if let openapiv3::ReferenceOr::Item(sub_schema) = sub {
                        if let openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) =
                            &sub_schema.schema_kind
                        {
                            return obj.properties.keys()
                                .find(|k| k.to_lowercase().contains(&term_lower))
                                .cloned();
                        }
                    }
                    None
                })
            }
            _ => None,
        };

        if let Some(field_name) = field_match {
            schemas.push(SchemaMatch {
                name,
                matched_field: Some(field_name),
            });
        }
    }
}
```

### Step 3 — Update the text renderer

In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`, find the schema rendering
block inside `render_search` (around line 694–699):

```rust
// Old:
if !results.schemas.is_empty() {
    out.push_str("\nSchemas:\n");
    for s in &results.schemas {
        writeln!(out, "  {}", sanitize(&s.name)).unwrap();
    }
}

// New:
if !results.schemas.is_empty() {
    out.push_str("\nSchemas:\n");
    for s in &results.schemas {
        match s.matched_field.as_deref() {
            Some(field) => writeln!(out, "  {} (field: {})", sanitize(&s.name), sanitize(field)).unwrap(),
            None => writeln!(out, "  {}", sanitize(&s.name)).unwrap(),
        }
    }
}
```

### Step 4 — Verify

Run: `cargo test -p phyllotaxis search_field_name`
Expected: **PASS**

Run full suite: `cargo test`
Expected: 200+ tests pass (additive change, no existing assertions broken).

Also confirm the annotation appears in search output — not required as a test, but good
to spot-check manually:

```
cargo run -- --spec tests/fixtures/kitchen-sink.yaml search email
```

Expected output includes:
```
Schemas:
  CreateUserRequest (field: email)
  PatchUserRequest (field: email)
  User (field: email)
```

### Step 5 — Commit

```
git add src/commands/search.rs src/render/text.rs
git commit -m "Search indexes schema field names, annotates match reason"
```

---

## Task 2: `--expand` inlines array-of-ref fields

**Design change #2** — `schemas.rs` only.

### Why this approach

The `expand_fields` function in `schemas.rs` already handles object `$ref` fields: it
checks `field.nested_schema_name`, resolves it, builds nested fields, and recurses.
Array-of-ref fields are currently built with `type_display = "ErrorDetail[]"` and
`nested_schema_name = Some("ErrorDetail")`, but `expand_fields` only populates
`nested_fields` when the field itself is an object ref — it does not distinguish array
refs. The fix: when expanding, if `nested_schema_name` is set, expand it regardless of
whether the field type is an array or object. The existing depth cap (`max_depth = 5`)
prevents infinite recursion.

Looking at `build_fields` in `resources.rs` (called from `schemas.rs`), array-of-ref
fields already have `nested_schema_name` set (the `$ref` target name). So the only
change needed is in `expand_fields`: remove the implicit assumption that only object
refs should be expanded.

Looking at the current `expand_fields` code: it already expands any field with
`nested_schema_name`, regardless of whether it's an array. The issue is that
`build_fields` for an array-of-ref field may or may not set `nested_schema_name`.

Let me verify: in `resources.rs`, the `build_field` function sets `nested_schema_name`
for `$ref` items. If it does, `expand_fields` will already expand it. If not, that's
the gap to close.

**Confirmed gap** (from manual test above): `schemas Error --expand` shows
`details  ErrorDetail[]` without nested fields. This means `nested_schema_name` is not
set for array-of-ref fields in `build_fields`, or `expand_fields` skips them.

The fix is in `expand_fields` in `schemas.rs`: the current implementation uses
`field.nested_schema_name` to drive expansion — we need to ensure array-of-ref fields
have this populated, OR ensure `expand_fields` also checks the array item ref.

Since `nested_schema_name` is already set for array-of-ref fields (the type display
shows `ErrorDetail[]`), the issue must be in `expand_fields`. Looking at the code:
`expand_fields` currently expands every field that has `nested_schema_name` set. But
the test shows `details  ErrorDetail[]` without expansion — so either `nested_schema_name`
is `None` for array fields, or `expand_fields` skips them.

We need to check `build_field` in `resources.rs`:

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

### Root cause

In `build_fields` (`resources.rs`, around line 172–225), properties are iterated from
`obj.properties`. For a direct `$ref` property (e.g. `owner: { $ref: '#/components/schemas/Owner' }`),
the match arm is `ReferenceOr::Reference { reference }`, which sets `schema_name = Some("Owner")`
and thus `nested_schema_name: schema_name.map(...)` is populated.

For an array-of-ref property (e.g. `details: { type: array, items: { $ref: '...' } }`),
the property itself is an inline schema (`ReferenceOr::Item(boxed)`), so `schema_name = None`
and `nested_schema_name = None`. The type display `"ErrorDetail[]"` is built in
`format_type_display`, which extracts the ref name — but that name never makes it back
to `nested_schema_name` on the `Field`.

The fix: in the `ReferenceOr::Item(boxed)` branch, after computing `type_display`, also
check if the resolved schema is an array whose items is a `$ref` — if so, extract the
ref name and set `nested_schema_name`.

### Step 2 — Write the failing test

Add to `schemas.rs` unit tests:

```rust
#[test]
fn test_expand_array_of_ref() {
    let api = load_kitchen_sink_api();
    // Error.details is array of ErrorDetail — should inline ErrorDetail fields when expanded
    let model = build_schema_model(&api, "Error", true, 5).unwrap();
    let details_field = model.fields.iter().find(|f| f.name == "details");
    assert!(details_field.is_some(), "Error should have a details field");
    let details = details_field.unwrap();
    assert!(
        !details.nested_fields.is_empty(),
        "With --expand, details (ErrorDetail[]) should have nested_fields populated. \
         Got type_display={:?}, nested_schema_name={:?}",
        details.type_display,
        details.nested_schema_name
    );
    // Spot-check that ErrorDetail's fields appear
    let field_names: Vec<&str> = details.nested_fields.iter().map(|f| f.name.as_str()).collect();
    assert!(
        field_names.contains(&"field") || field_names.contains(&"reason"),
        "Expanded details should contain ErrorDetail fields (field, reason), got: {:?}",
        field_names
    );
}
```

Run: `cargo test -p phyllotaxis test_expand_array_of_ref`
Expected: **FAIL**

### Step 3 — Implement

In `build_fields` in `src/commands/resources.rs`, find the property iteration loop.
The `ReferenceOr::Item(boxed)` branch (around line 175) currently sets
`schema_name = None`. After computing `type_display`, extract the array item ref name
if applicable:

```rust
// Old: after the resolved_schema / schema_name match block (around line 196):
let type_display = if let Some(sname) = schema_name {
    sname.to_string()
} else {
    format_type_display(&resolved.schema_kind)
};

// ... later:
fields.push(Field {
    // ...
    nested_schema_name: schema_name.map(|s| s.to_string()),
    // ...
});

// New: extract array item ref name when schema_name is None but the field is array-of-ref
let type_display = if let Some(sname) = schema_name {
    sname.to_string()
} else {
    format_type_display(&resolved.schema_kind)
};

// For array-of-ref fields, capture the item schema name for expand support
let array_item_schema_name: Option<String> = if schema_name.is_none() {
    if let openapiv3::SchemaKind::Type(openapiv3::Type::Array(arr)) = &resolved.schema_kind {
        if let Some(openapiv3::ReferenceOr::Reference { reference }) = &arr.items {
            spec::schema_name_from_ref(reference.as_str()).map(|s| s.to_string())
        } else {
            None
        }
    } else {
        None
    }
} else {
    None
};

// ... later in the Field { ... } constructor:
fields.push(Field {
    name: name.clone(),
    type_display,
    required: required_fields.contains(name),
    optional: !required_fields.contains(name),
    nullable: resolved.schema_data.nullable,
    read_only: resolved.schema_data.read_only,
    write_only: resolved.schema_data.write_only,
    deprecated: resolved.schema_data.deprecated,
    description: resolved.schema_data.description.clone(),
    enum_values,
    constraints: extract_constraints(&resolved.schema_kind),
    default_value: resolved.schema_data.default.clone(),
    example: resolved.schema_data.example.clone(),
    nested_schema_name: schema_name.map(|s| s.to_string()).or(array_item_schema_name),
    nested_fields: Vec::new(),
});
```

With `nested_schema_name` now set for array-of-ref fields, the existing `expand_fields`
logic in `schemas.rs` will pick it up automatically and populate `nested_fields`.

### Step 4 — Verify

Run: `cargo test -p phyllotaxis test_expand_array_of_ref`
Expected: **PASS**

Add an integration test to `tests/integration_tests.rs`:

```rust
#[test]
fn test_expand_inlines_array_ref_fields() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "Error", "--expand"]);
    assert_eq!(code, 0);
    // After expansion, details field should show ErrorDetail's sub-fields
    assert!(
        stdout.contains("field") && stdout.contains("reason"),
        "Expanded Error.details should show ErrorDetail fields (field, reason), got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

Run: `cargo test test_expand_inlines_array_ref_fields`
Expected: **PASS**

Run full suite: `cargo test`
Expected: all pass.

### Step 5 — Commit

```
git add src/commands/resources.rs tests/integration_tests.rs
git commit -m "Expand inlines array-of-ref fields (ErrorDetail[] now shows nested fields)"
```

---

## Task 3: Search results show match reason

**Design change #3** — `search.rs` (struct + logic), `text.rs` (rendering),
`json.rs` (JSON output gets new field automatically via serde derive).

### Why this approach

`EndpointMatch` currently has no way to signal why an endpoint matched. Adding
`matched_on: Option<String>` as an optional serde field is purely additive: existing
JSON consumers see a new field only when it's populated (and we use
`skip_serializing_if = "Option::is_none"` to avoid polluting results where it's not
relevant). The logic change is minimal: the parameter match loop already knows which
parameter triggered the match — we capture the first matching parameter name.

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs`
- `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

(`json.rs` picks up `matched_on` automatically since `SearchResults` and its children
derive `serde::Serialize`.)

### Step 1 — Write the failing test

Add to `search.rs` unit tests:

```rust
#[test]
fn test_search_endpoint_match_reason_parameter() {
    let api = load_kitchen_sink_api();
    // session_token is a cookie parameter on GET /users — not in path/summary/description
    let results = search(&api, "session");
    assert!(
        !results.endpoints.is_empty(),
        "Search for 'session' should find GET /users via session_token param"
    );
    let users_get = results
        .endpoints
        .iter()
        .find(|e| e.path == "/users" && e.method == "GET");
    assert!(
        users_get.is_some(),
        "Expected GET /users in results, got: {:?}",
        results.endpoints.iter().map(|e| (&e.method, &e.path)).collect::<Vec<_>>()
    );
    assert_eq!(
        users_get.unwrap().matched_on.as_deref(),
        Some("parameter: session_token"),
        "matched_on should be 'parameter: session_token'"
    );
}

#[test]
fn test_search_endpoint_match_reason_none_for_path_match() {
    let api = load_kitchen_sink_api();
    // /users matches by path for "users" — matched_on should be None
    let results = search(&api, "users");
    let path_match = results
        .endpoints
        .iter()
        .find(|e| e.path == "/users" && e.method == "GET");
    if let Some(m) = path_match {
        assert!(
            m.matched_on.is_none(),
            "Path-matched endpoint should have matched_on=None, got: {:?}",
            m.matched_on
        );
    }
}
```

Run: `cargo test -p phyllotaxis test_search_endpoint_match_reason`
Expected: **FAIL** (field `matched_on` does not exist on `EndpointMatch`)

### Step 2 — Implement

**In `search.rs`**, update `EndpointMatch`:

```rust
// Old:
#[derive(Debug, serde::Serialize)]
pub struct EndpointMatch {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub resource_slug: String,
}

// New:
#[derive(Debug, serde::Serialize)]
pub struct EndpointMatch {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub resource_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_on: Option<String>,
}
```

Update the endpoint search block to capture the first matching parameter name. Replace
the `param_match` bool computation and the `endpoints.push` call:

```rust
// Replace the param_match + push block:
let mut matched_param_name: Option<String> = None;
for p in &op.parameters {
    if let openapiv3::ReferenceOr::Item(param) = p {
        let pdata = match param {
            openapiv3::Parameter::Query { parameter_data, .. } => parameter_data,
            openapiv3::Parameter::Path { parameter_data, .. } => parameter_data,
            openapiv3::Parameter::Header { parameter_data, .. } => parameter_data,
            openapiv3::Parameter::Cookie { parameter_data, .. } => parameter_data,
        };
        let name_match = pdata.name.to_lowercase().contains(&term_lower);
        let desc_match = pdata
            .description
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&term_lower);
        if name_match || desc_match {
            matched_param_name = Some(pdata.name.clone());
            break;
        }
    }
}
let param_match = matched_param_name.is_some();

if path_match || summary_match || desc_match || param_match {
    let resource_slug = op
        .tags
        .first()
        .map(|t| slugify(t))
        .unwrap_or_default();

    // Only annotate matched_on when the match came from a parameter,
    // and the match was NOT also a path/summary/description match.
    let matched_on = if param_match && !path_match && !summary_match && !desc_match {
        matched_param_name.map(|n| format!("parameter: {}", n))
    } else {
        None
    };

    endpoints.push(EndpointMatch {
        method: method.to_string(),
        path: path_str.clone(),
        summary: op.summary.clone(),
        resource_slug,
        matched_on,
    });
}
```

Note: the local variable `desc_match` is used both for the operation description and as a
name in the parameter loop — rename the operation-level one to avoid shadowing. Replace
`desc_match` (operation description) with `op_desc_match` throughout this block.

**In `text.rs`**, update the endpoint rendering in `render_search` to show the annotation.
Find the endpoint rendering loop (around line 675–691):

```rust
// Old:
for e in &results.endpoints {
    let summary = sanitize(e.summary.as_deref().unwrap_or(""));
    writeln!(
        out,
        "  {:<7} {:<width$}  {}",
        sanitize(&e.method), sanitize(&e.path), summary, width = max_path
    ).unwrap();
    if !e.resource_slug.is_empty() {
        writeln!(
            out,
            "    phyllotaxis resources {} {} {}",
            sanitize(&e.resource_slug),
            sanitize(&e.method),
            sanitize(&e.path),
        ).unwrap();
    }
}

// New:
for e in &results.endpoints {
    let summary = sanitize(e.summary.as_deref().unwrap_or(""));
    let reason = e.matched_on.as_ref()
        .map(|r| format!("  ({})", sanitize(r)))
        .unwrap_or_default();
    writeln!(
        out,
        "  {:<7} {:<width$}  {}{}",
        sanitize(&e.method), sanitize(&e.path), summary, reason, width = max_path
    ).unwrap();
    if !e.resource_slug.is_empty() {
        writeln!(
            out,
            "    phyllotaxis resources {} {} {}",
            sanitize(&e.resource_slug),
            sanitize(&e.method),
            sanitize(&e.path),
        ).unwrap();
    }
}
```

### Step 3 — Verify

Run: `cargo test -p phyllotaxis test_search_endpoint_match_reason`
Expected: **PASS**

Add an integration test in `tests/integration_tests.rs`:

```rust
#[test]
fn test_search_shows_match_reason_for_param_match() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["search", "session_token"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("(parameter: session_token)"),
        "Search result for param match should show match reason, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_search_json_includes_matched_on() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--json", "search", "session_token"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", &stdout[..200.min(stdout.len())]));
    let endpoints = json["endpoints"].as_array().expect("endpoints array");
    let has_matched_on = endpoints.iter().any(|e| {
        e.get("matched_on")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with("parameter:"))
            .unwrap_or(false)
    });
    assert!(has_matched_on, "JSON endpoint match should have matched_on field for param match");
}
```

Run: `cargo test test_search_shows_match_reason`
Expected: **PASS**

Run full suite: `cargo test`
Expected: all pass.

### Step 4 — Commit

```
git add src/commands/search.rs src/render/text.rs tests/integration_tests.rs
git commit -m "Add match reason annotation to search results (parameter: <name>)"
```

---

## Task 4: Suppress empty parameter sections + field alignment fix

**Design changes #4 and #8** — `text.rs` only. Combined because both are isolated
rendering changes in the same file with no model impact.

### Why this approach

**Change #4 (suppress empty sections):** The `render_param_section` function
unconditionally writes the section header then checks if `params.is_empty()`. Flipping
the guard so the section is skipped entirely when empty fixes the UX issue. Header
params are already conditionally rendered (the `if !header_params.is_empty()` guard
exists at line 131). Path and query params need the same treatment.

**Change #8 (field alignment):** The `render_fields_section` and `render_schema_fields`
functions use `{:<tw$}` for the type column but leave constraints as a trailing
suffix appended without padding. When a field's type is short (e.g. `string`) and
another field's type is long (e.g. `string/password`), the constraint column (`min:8`)
appears at a different horizontal position. The fix: compute a `max_constraints_len`
across the field list (using the joined constraint string) and pad it as a fixed-width
column, matching the pattern already used for name and type.

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

### Step 1 — Write the failing tests

Add to `tests/integration_tests.rs`:

```rust
#[test]
fn test_empty_path_params_section_suppressed() {
    // POST /users has no path parameters — section should not appear
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "POST", "/users"]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains("Path Parameters:"),
        "POST /users has no path params — 'Path Parameters:' section should not appear, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_empty_query_params_section_suppressed() {
    // POST /users has no query parameters — section should not appear
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "POST", "/users"]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains("Query Parameters:"),
        "POST /users has no query params — 'Query Parameters:' section should not appear, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_non_empty_path_params_still_shown() {
    // GET /users/{userId} has a path parameter — section should still appear
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "GET", "/users/{userId}"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Path Parameters:"),
        "Endpoint with path params should still show section, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_constraint_column_alignment() {
    // POST /users request body has fields with different constraint sets.
    // Check that the constraints column starts at the same position for all rows.
    // Strategy: find "min:3" and "min:8" lines; they should have the same column offset.
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "POST", "/users"]);
    assert_eq!(code, 0);

    let min3_line = stdout.lines().find(|l| l.contains("min:3"));
    let min8_line = stdout.lines().find(|l| l.contains("min:8"));

    assert!(min3_line.is_some(), "Expected line with min:3 constraint");
    assert!(min8_line.is_some(), "Expected line with min:8 constraint");

    // Find the column position of "min:" in each line
    let pos3 = min3_line.unwrap().find("min:").unwrap();
    let pos8 = min8_line.unwrap().find("min:").unwrap();
    assert_eq!(
        pos3, pos8,
        "min:3 and min:8 should start at the same column.\nmin:3 line: {:?}\nmin:8 line: {:?}",
        min3_line.unwrap(),
        min8_line.unwrap()
    );
}
```

Run: `cargo test test_empty_path_params_section_suppressed test_empty_query_params_section_suppressed test_constraint_column_alignment`
Expected: **FAIL**

### Step 2 — Implement: suppress empty param sections

In `text.rs`, find `render_endpoint_detail`. The current calls to `render_param_section`
are unconditional for path and query params:

```rust
// Old (around line 129–133):
render_param_section(&mut out, "Path Parameters", &path_params);
render_param_section(&mut out, "Query Parameters", &query_params);
if !header_params.is_empty() {
    render_param_section(&mut out, "Header Parameters", &header_params);
}

// New:
if !path_params.is_empty() {
    render_param_section(&mut out, "Path Parameters", &path_params);
}
if !query_params.is_empty() {
    render_param_section(&mut out, "Query Parameters", &query_params);
}
if !header_params.is_empty() {
    render_param_section(&mut out, "Header Parameters", &header_params);
}
```

Also remove the `(none)` branch from `render_param_section` since it's now unreachable:

```rust
// Old:
fn render_param_section(
    out: &mut String,
    title: &str,
    params: &[&crate::models::resource::Parameter],
) {
    writeln!(out, "\n{}:", title).unwrap();
    if params.is_empty() {
        out.push_str("  (none)\n");
        return;
    }
    // ... rest of function

// New:
fn render_param_section(
    out: &mut String,
    title: &str,
    params: &[&crate::models::resource::Parameter],
) {
    writeln!(out, "\n{}:", title).unwrap();
    let max_name = params.iter().map(|p| p.name.len()).max().unwrap_or(0);
    // ... rest of function (removing the is_empty check)
```

### Step 3 — Implement: field alignment fix

The alignment issue is in both `render_fields_section` (used for request body) and
`render_schema_fields` (used for schema detail). Both have the same structure.

The current format string is:
```rust
"  {:<nw$}  {:<tw$}  {:<20}  {}{}{}",
//  name     type     flags    desc constraints enums
```

The `constraints_str` is appended after `desc` without alignment — it starts at
different columns depending on how long `desc` is. The correct fix: rather than
appending constraints to the description, add a separate aligned column.

New format approach: compute `max_constraint_width` across all fields in the section,
then use it as a fixed column width before the description:

```rust
fn render_fields_section(out: &mut String, fields: &[crate::models::resource::Field]) {
    if fields.is_empty() {
        return;
    }
    let max_name = fields.iter().map(|f| f.name.len()).max().unwrap_or(0);
    let max_type = fields.iter().map(|f| f.type_display.len()).max().unwrap_or(0);
    let max_constraint = fields
        .iter()
        .map(|f| if f.constraints.is_empty() { 0 } else { f.constraints.join(" ").len() })
        .max()
        .unwrap_or(0);

    for f in fields {
        // ... (same flags/enums/desc building as before) ...

        let constraints_str = f.constraints.join(" ");

        if !f.nested_fields.is_empty() {
            writeln!(
                out,
                "  {:<nw$}  {}:",
                sanitize(&f.name),
                sanitize(&f.type_display),
                nw = max_name,
            ).unwrap();
            render_schema_fields(out, &f.nested_fields, 4);
            continue;
        }

        writeln!(
            out,
            "  {:<nw$}  {:<tw$}  {:<20}  {:<cw$}  {}{}",
            sanitize(&f.name),
            sanitize(&f.type_display),
            flag_str,
            constraints_str,
            desc,
            enums,
            nw = max_name,
            tw = max_type,
            cw = max_constraint,
        ).unwrap();
    }
}
```

Apply the same change to `render_schema_fields`, which has the same structure but uses a
`prefix` variable for indentation.

**Important:** The old format string appended `constraints_str` after `desc` as `{}{}{}`.
The new format puts constraints before `desc` as a padded column. This changes the column
order to:

```
  <name>  <type>  <flags>  <constraints>  <description>  <enums>
```

This is cleaner than the old layout where constraints appeared after the description.

### Step 4 — Verify

Run: `cargo test test_empty_path_params_section_suppressed test_empty_query_params_section_suppressed test_non_empty_path_params_still_shown test_constraint_column_alignment`
Expected: **PASS**

Also check that existing constraint tests still pass:

```
cargo test test_schema_constraints_visible
cargo test test_write_only_visible_on_create_user_request
```

Run full suite: `cargo test`
Expected: all pass.

**Gotcha:** Existing tests that assert `stdout.contains("Path Parameters:")` will break
if those endpoints actually have no path params. Check `test_resources_endpoint_get` —
it tests `GET /pets` which has no path params. That test will now need updating, or
`GET /pets` needs path params (it doesn't). After implementing, update the test:

```rust
// test_resources_endpoint_get currently asserts:
assert!(stdout.contains("Query Parameters"), "Missing query parameters section");
// The GET /pets endpoint does have query params (limit), so this still passes.
// Path params would only be shown if the endpoint has them.
```

Verify by running:
```
cargo test test_resources_endpoint_get
```

If it fails because the test also asserts on Path Parameters, adjust that test to not
assert on the path params section.

### Step 5 — Commit

```
git add src/render/text.rs tests/integration_tests.rs
git commit -m "Suppress empty param sections; align constraint column in field tables"
```

---

## Task 5: Search result counts + consistent drill-deeper hints

**Design changes #5 and #6** — `text.rs` (rendering) and `json.rs` (JSON counts).
Combined because both are small additions to the same render functions.

### Why this approach

**Change #6 (counts):** Add a summary line at the start of search results. The
`SearchResults` struct already has all counts available as `.len()` calls. In JSON,
adding top-level count fields is straightforward — `SearchResults` derives `Serialize`
so we can either add fields directly to the struct or create a wrapper. Since the struct
is already serialized directly via `serialize(results, is_tty)` in `json.rs`, the
cleanest approach is to add `endpoint_count` and `schema_count` fields to `SearchResults`
itself (computed at search time, not render time).

**Change #5 (auth drill-deeper):** The auth view currently has a generic drill-deeper
hint: "phyllotaxis resources    Browse endpoints by resource group". The design doc
wants per-scheme hints: "phyllotaxis resources <name> to see endpoints using a scheme".
This requires knowing the resource slugs that use each scheme, which `AuthModel` doesn't
currently contain. The simpler interpretation is a single hint that directs the LLM to
filter by auth scheme — but the auth model doesn't have that data. The most practical
fix: change the existing auth drill-deeper hint text to be more informative, and add an
explicit note that resources can be filtered by scheme via search.

Actually, re-reading the design doc: "Auth view → `phyllotaxis resources <name>` to see
endpoints using a scheme". This likely means a generic hint like:
"phyllotaxis search <scheme_name>" to find endpoints using the scheme, not per-resource
navigation. Use the scheme names already in the auth model.

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs` (add count fields to struct)
- `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (summary line + auth hint)
- `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs` (counts in JSON — via struct)

### Step 1 — Write the failing tests

Add to `tests/integration_tests.rs`:

```rust
#[test]
fn test_search_shows_result_counts() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["search", "user"]);
    assert_eq!(code, 0);
    // Should have a summary line at the top
    assert!(
        stdout.contains("Found"),
        "Search results should start with a 'Found N ...' summary, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
    // Should mention endpoint and schema counts
    assert!(
        stdout.contains("endpoint") || stdout.contains("Endpoint"),
        "Summary should mention endpoints, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
    assert!(
        stdout.contains("schema") || stdout.contains("Schema"),
        "Summary should mention schemas, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

#[test]
fn test_search_json_includes_counts() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--json", "search", "user"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", &stdout[..200.min(stdout.len())]));
    assert!(
        json.get("endpoint_count").is_some(),
        "JSON search results should include endpoint_count"
    );
    assert!(
        json.get("schema_count").is_some(),
        "JSON search results should include schema_count"
    );
}

#[test]
fn test_auth_drill_deeper_shows_search_hints() {
    // TTY check: run with a helper that forces is_tty=true.
    // The auth drill-deeper hint should mention searching by scheme name.
    // Since integration tests run non-TTY, we test via the text renderer unit test instead.
    // (See unit test in text.rs below.)
}
```

For the auth drill-deeper and schema listing drill-deeper, add unit tests in `text.rs` module tests:

```rust
#[test]
fn test_auth_drill_deeper_shows_scheme_search_hints() {
    use crate::commands::auth::{AuthModel, SecuritySchemeInfo};

    let model = AuthModel {
        schemes: vec![
            SecuritySchemeInfo {
                name: "bearerAuth".to_string(),
                scheme_type: "http".to_string(),
                detail: "bearer".to_string(),
                description: None,
                usage_count: 5,
            },
        ],
        total_operations: 10,
    };

    let output = render_auth(&model, true); // is_tty = true
    assert!(
        output.contains("phyllotaxis search bearerAuth"),
        "Auth drill-deeper should suggest searching by scheme name. Got:\n{}",
        output
    );
}

#[test]
fn test_schema_listing_drill_deeper_hint() {
    // Schema listing drill-deeper hint: "phyllotaxis schemas <name>"
    // Since integration tests run non-TTY, test via text renderer unit test.
    // Build a minimal SchemaListModel with one schema name and render it with is_tty=true.
    use crate::commands::schemas::SchemaListModel;

    let model = SchemaListModel {
        schemas: vec!["User".to_string(), "Error".to_string()],
    };

    let output = render_schema_list(&model, true); // is_tty = true
    assert!(
        output.contains("phyllotaxis schemas"),
        "Schema listing should show drill-deeper hint with 'phyllotaxis schemas <name>'. Got:\n{}",
        output
    );
}
```

Add an integration test in `tests/integration_tests.rs` for schema listing drill-deeper:

```rust
#[test]
fn test_schema_listing_shows_drill_deeper_hint() {
    // TTY output check: schemas list should end with drill-deeper hint.
    // The hint only appears when is_tty=true. The integration test runner is non-TTY,
    // so this is covered by the unit test above. This test verifies the non-TTY path
    // does NOT show the hint (matches existing behavior pattern).
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas"]);
    assert_eq!(code, 0);
    // Non-TTY: no drill-deeper (consistent with all other commands)
    assert!(
        !stdout.contains("Drill deeper:"),
        "Non-TTY schema listing should not show drill-deeper hints, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}
```

Run: `cargo test test_search_shows_result_counts test_search_json_includes_counts test_auth_drill_deeper_shows_scheme_search_hints test_schema_listing_drill_deeper_hint`
Expected: **FAIL**

### Step 2 — Implement: result counts

**In `search.rs`**, add count fields to `SearchResults`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct SearchResults {
    pub term: String,
    pub endpoint_count: usize,
    pub schema_count: usize,
    pub resources: Vec<ResourceMatch>,
    pub endpoints: Vec<EndpointMatch>,
    pub schemas: Vec<SchemaMatch>,
    pub callbacks: Vec<CallbackMatch>,
}
```

Update the `SearchResults { ... }` constructor at the end of `search()`:

```rust
SearchResults {
    term: term.to_string(),
    endpoint_count: endpoints.len(),
    schema_count: schemas.len(),
    resources,
    endpoints,
    schemas,
    callbacks,
}
```

Update any existing test construction of `SearchResults` in `json.rs` unit tests (the
`test_all_json_outputs_parse` test constructs a `SearchResults` manually — add the new
fields with value `0`):

```rust
let results = SearchResults {
    term: "test".to_string(),
    endpoint_count: 0,
    schema_count: 0,
    resources: vec![],
    endpoints: vec![],
    schemas: vec![],
    callbacks: vec![],
};
```

**In `text.rs`**, add the summary line at the top of `render_search`, after the
`has_any` check:

```rust
// After "Results for ..."
writeln!(out, "Results for \"{}\":", results.term).unwrap();

// Add summary line:
if results.endpoint_count > 0 || results.schema_count > 0 {
    let mut parts = Vec::new();
    if results.endpoint_count > 0 {
        let label = if results.endpoint_count == 1 { "endpoint" } else { "endpoints" };
        parts.push(format!("{} {}", results.endpoint_count, label));
    }
    if results.schema_count > 0 {
        let label = if results.schema_count == 1 { "schema" } else { "schemas" };
        parts.push(format!("{} {}", results.schema_count, label));
    }
    writeln!(out, "Found {} matching \"{}\".", parts.join(", "), sanitize(&results.term)).unwrap();
}
```

### Step 3 — Implement: auth drill-deeper hints

In `text.rs`, find `render_auth`. The current TTY drill-deeper block (around line 769):

```rust
// Old:
if is_tty {
    out.push_str("\nDrill deeper:\n");
    out.push_str("  phyllotaxis resources    Browse endpoints by resource group\n");
}

// New:
if is_tty {
    out.push_str("\nDrill deeper:\n");
    for scheme in &model.schemes {
        writeln!(
            out,
            "  phyllotaxis search {}    Find endpoints using this scheme",
            sanitize(&scheme.name)
        ).unwrap();
    }
}
```

### Step 4 — Implement: schema listing drill-deeper hint

In `text.rs`, find `render_schema_list` (the function that renders `phyllotaxis schemas`
with no name argument — the listing view). Add a TTY drill-deeper hint at the end:

```rust
// Old (at end of render_schema_list):
// [no drill-deeper hint]

// New:
if is_tty {
    out.push_str("\nDrill deeper:\n");
    out.push_str("  phyllotaxis schemas <name>    Show fields and composition for a schema\n");
}
```

**Note:** `render_schema_list` must accept an `is_tty: bool` parameter for this to work.
Check whether it currently does. If not, add the parameter and update the call site in
the dispatch code (likely `main.rs` or the command dispatch).

### Step 5 — Verify

Run: `cargo test test_search_shows_result_counts test_search_json_includes_counts test_auth_drill_deeper_shows_scheme_search_hints test_schema_listing_drill_deeper_hint`
Expected: **PASS**

Run full suite: `cargo test`
Expected: all pass.

**Gotcha:** `test_auth` in `integration_tests.rs` currently passes for non-TTY output
and does not assert on drill-deeper hints, so no update needed there. The
`test_no_color_env_plain_output` and `test_piped_output_plain` tests assert that
`"Drill deeper:"` is absent in non-TTY — these are unaffected since both auth and schema
listing changes are gated on `is_tty`.

### Step 6 — Commit

```
git add src/commands/search.rs src/render/text.rs src/render/json.rs tests/integration_tests.rs
git commit -m "Add search result counts; add drill-deeper hints to schema listing and auth views"
```

---

## Task 6: NonAdminRole base type display

**Design change #7** — `models/schema.rs` (new field), `schemas.rs` (populate it),
`text.rs` (render it), `json.rs` (included automatically via serde).

### Why this approach

The design doc says: "When a schema has no fields and no composition, extract the
underlying type from the OpenAPI schema (string, integer, etc.) and display it."

`NonAdminRole` in the fixture uses `not:`, which produces `SchemaKind::Not { .. }` — a
variant not matched by the existing `build_schema_model` arms. This results in
`(fields=[], composition=None)`, so the schema renders with just its name and
description. The fix: handle `SchemaKind::Not` and other unrecognized kinds by extracting
a human-readable "base type" label.

For `not:` specifically, there is no simple primitive type to extract — but we can label
it as `not (object)` or just `not`. For true primitive type aliases (e.g.
`type: string` at the top level), we can extract `"string"`, `"integer"`, etc.

Since `NonAdminRole` uses `not:`, the display target is:
`Schema: NonAdminRole` → `Schema: NonAdminRole (not)` with description.

More generally, add `base_type: Option<String>` to `SchemaModel` and populate it in
`build_schema_model` for any `SchemaKind` branch that produces no fields and no
composition:

- `SchemaKind::Type(Type::String)` (non-enum): `"string"`
- `SchemaKind::Type(Type::Integer)`: `"integer"`
- `SchemaKind::Type(Type::Number)`: `"number"`
- `SchemaKind::Type(Type::Boolean)`: `"boolean"`
- `SchemaKind::Type(Type::Array)` (no ref items): `"array"`
- `SchemaKind::Not { .. }`: `"not"`

### Files to modify

- `/home/hhewett/.local/src/phyllotaxis/src/models/schema.rs`
- `/home/hhewett/.local/src/phyllotaxis/src/commands/schemas.rs`
- `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`
- `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

### Step 1 — Write the failing test

Add to `schemas.rs` unit tests:

```rust
#[test]
fn test_non_admin_role_has_base_type() {
    let api = load_kitchen_sink_api();
    let model = build_schema_model(&api, "NonAdminRole", false, 5).unwrap();
    assert!(
        model.base_type.is_some(),
        "NonAdminRole should have base_type set (it's a 'not' schema), got None"
    );
    assert_eq!(
        model.base_type.as_deref(),
        Some("not"),
        "NonAdminRole base_type should be 'not'"
    );
}
```

Add an integration test in `tests/integration_tests.rs`:

```rust
#[test]
fn test_non_admin_role_shows_base_type() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "NonAdminRole"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("NonAdminRole (not)") || stdout.contains("Type: not"),
        "NonAdminRole schema should display its base type, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}
```

Run: `cargo test test_non_admin_role_has_base_type test_non_admin_role_shows_base_type`
Expected: **FAIL**

### Step 2 — Implement: model field

**In `models/schema.rs`**, add `base_type`:

```rust
// Old:
#[derive(Debug, serde::Serialize)]
pub struct SchemaModel {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub discriminator: Option<DiscriminatorInfo>,
    pub external_docs: Option<super::resource::ExternalDoc>,
}

// New:
#[derive(Debug, serde::Serialize)]
pub struct SchemaModel {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub base_type: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub discriminator: Option<DiscriminatorInfo>,
    pub external_docs: Option<super::resource::ExternalDoc>,
}
```

### Step 3 — Implement: population in `schemas.rs`

In `build_schema_model`, the `(fields, composition)` match arm has a catch-all `_ => (Vec::new(), None)`. Extend the function to derive `base_type` from the schema kind:

After the `let (fields, composition) = match ...` block (around line 82–120), add:

```rust
let base_type = if fields.is_empty() && composition.is_none() {
    match &schema.schema_kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(_)) => Some("string".to_string()),
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(_)) => Some("integer".to_string()),
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(_)) => Some("number".to_string()),
        openapiv3::SchemaKind::Type(openapiv3::Type::Boolean { .. }) => Some("boolean".to_string()),
        openapiv3::SchemaKind::Type(openapiv3::Type::Array(_)) => Some("array".to_string()),
        openapiv3::SchemaKind::Not { .. } => Some("not".to_string()),
        _ => None,
    }
} else {
    None
};
```

Update the `Some(SchemaModel { ... })` return value to include `base_type`:

```rust
Some(SchemaModel {
    name: name.to_string(),
    title,
    description,
    base_type,
    fields,
    composition,
    discriminator,
    external_docs: None,
})
```

**Note:** The `base_type` for `String` enum schemas will not be set because those go
through the enum branch (`composition = Some(Composition::Enum(...))` and are not caught
by the `fields.is_empty() && composition.is_none()` guard). This is correct — enums
already display their values.

### Step 4 — Implement: text rendering

In `text.rs`, update `render_schema_detail`. The header section currently:

```rust
// Old:
if expanded {
    writeln!(out, "Schema: {} (expanded)", sanitize(&model.name)).unwrap();
} else {
    writeln!(out, "Schema: {}", sanitize(&model.name)).unwrap();
}

// New:
let base_type_suffix = model.base_type.as_ref()
    .map(|t| format!(" ({})", sanitize(t)))
    .unwrap_or_default();
if expanded {
    writeln!(out, "Schema: {} (expanded){}", sanitize(&model.name), base_type_suffix).unwrap();
} else {
    writeln!(out, "Schema: {}{}", sanitize(&model.name), base_type_suffix).unwrap();
}
```

### Step 5 — Update existing unit tests for `SchemaModel` construction

Any test code that constructs `SchemaModel` directly needs `base_type: None` added.
Locations:

1. `src/render/json.rs` — `test_all_json_outputs_parse` constructs `SchemaModel` twice.
   Add `base_type: None` to both.

2. Any other `SchemaModel { ... }` literals in the test suite.

Search: `grep -rn "SchemaModel {" src/ tests/`

### Step 6 — Verify

Run: `cargo test test_non_admin_role_has_base_type test_non_admin_role_shows_base_type`
Expected: **PASS**

Run full suite: `cargo test`
Expected: all pass.

Also verify JSON includes `base_type`:

```bash
cargo run -- --spec tests/fixtures/kitchen-sink.yaml --json schemas NonAdminRole | python3 -m json.tool | grep base_type
```

Expected: `"base_type": "not"`

### Step 7 — Commit

```
git add src/models/schema.rs src/commands/schemas.rs src/render/text.rs src/render/json.rs tests/integration_tests.rs
git commit -m "Display base type for schemas without fields or composition (NonAdminRole shows 'not')"
```

---

## Final verification

After all six tasks are complete:

```bash
cargo test
```

Expected: all tests pass (200+ from baseline, plus ~15 new tests added across tasks).

Spot-check the two highest-priority fixes manually:

```bash
# Change #1: field search
cargo run -- --spec tests/fixtures/kitchen-sink.yaml search email
# Expected: User (field: email), CreateUserRequest (field: email), PatchUserRequest (field: email)

# Change #2: array-ref expand
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas Error --expand
# Expected: details field shows ErrorDetail sub-fields (field, reason, value)
```

---

## Implementation notes

### Task execution order

Tasks 1 and 2 are independent — either can go first. Tasks 3–6 are also independent of
each other but all depend on a passing baseline, so do them after Tasks 1 and 2.

### Variable name shadowing in Task 3

`search.rs` currently has a local variable `desc_match` for both the operation
description check and the parameter description check. When refactoring the parameter
loop, rename the operation-level `desc_match` to `op_desc_match` to avoid confusion:

```rust
let op_desc_match = op
    .description
    .as_deref()
    .unwrap_or("")
    .to_lowercase()
    .contains(&term_lower);
// ... later:
if path_match || summary_match || op_desc_match || param_match {
```

### Column order change in Task 4

Changing constraint placement from after-description to before-description is a visual
change. Existing tests that assert `stdout.contains("min:3")` will still pass since the
value is still present. Tests that assert on exact column positions will need updating
(none currently exist).

### `base_type` for primitive type aliases

The `base_type` field is only set when there are no fields AND no composition. This means
a schema like:
```yaml
MyString:
  type: string
  description: A simple string alias
```
would show `Schema: MyString (string)`. This is the intended behavior from the design doc.
However, if a string schema has format (e.g. `format: email`), the `base_type` will still
be `"string"` — it does not include format. This is acceptable for the current scope.

### JSON backward compatibility

All new fields use `#[serde(skip_serializing_if = "Option::is_none")]` where they are
`Option<String>`. This means:
- Existing JSON consumers see no change when `matched_field` or `matched_on` is absent.
- `endpoint_count` and `schema_count` in `SearchResults` are always present (they are
  `usize`, not `Option`). This is a minor breaking change for JSON consumers that
  validate schema strictly — acceptable since the tool is currently in early use.
- `base_type` on `SchemaModel` is `Option<String>` and will be omitted when `None`.

### The `test_resources_endpoint_get` guard

`test_resources_endpoint_get` currently asserts:
```rust
assert!(stdout.contains("Query Parameters"), "Missing query parameters section");
```
`GET /pets` has a `limit` query parameter, so this assertion still passes after change #4.
No update needed. However, if the petstore `GET /pets` endpoint ever loses its query
params, this test would need updating.
