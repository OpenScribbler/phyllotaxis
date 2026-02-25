# Kitchen Sink Coverage Gaps — Implementation Plan

**Design doc:** `docs/plans/2026-02-23-kitchen-sink-coverage-gaps-design.md`
**Date:** 2026-02-23

---

## Dependency Graph

```
Phase 1: Model Changes (Tasks 1–5)
  Task 1 (Field extensions: write_only, deprecated, constraints)
  Task 2 (ResponseHeader model + headers on Response)
  Task 3 (Link/ResponseLink models)
  Task 4 (CallbackOperation/CallbackEntry models)
  Task 5 (SchemaDetail title on SchemaModel)

  NOTE: ALL of Tasks 1, 2, 3, 4, 5 must complete before running cargo check.
  Each one extends a struct with new fields, which breaks all existing struct-literal
  construction sites in tests and commands. Construction sites will fail to compile
  from ALL five tasks — not just Task 1. Run cargo check once after Task 5, fix all
  sites in one pass.

Phase 2: Extraction Changes (Tasks 6–13)
  Task 6  — depends on Task 1: extract write_only, deprecated into build_fields
  Task 7  — depends on Task 1: extract constraints into build_fields
  Task 8  — depends on Task 1: fix integer/number enum extraction
  Task 9  — depends on Task 2, Task 3: extract response headers in extract_responses
  Task 10 — depends on Task 3: extract links from responses in get_endpoint_detail
  Task 11 — depends on Task 4: extract callbacks inline on Endpoint
  Task 12 — depends on Task 4: new callbacks extraction module (commands/callbacks.rs)
  Task 13 — depends on Task 5: extract title in build_schema_model

Phase 3: Request Body Overhaul (Task 14)
  Task 14 — depends on Tasks 6, 7, 8: multi-content-type request body extraction

Phase 4: Rendering (Tasks 15–22)
  Task 15 — depends on Tasks 6+7+8: render write_only/deprecated/constraints/int-enums in text fields
  Task 16 — depends on Task 4, Task 9: render response headers in text renderer
  Task 17 — depends on Task 4, Task 10: render links in text renderer
  Task 18 — depends on Task 4, Task 11: render callbacks inline in text renderer
  Task 19 — depends on Task 13: render schema title in text renderer
  Task 20 — depends on Tasks 6+7+8: update JSON FieldJson struct + convert_fields
  Task 21 — depends on Tasks 9+10+11: update JSON endpoint_detail renderer
  Task 22 — depends on Task 12: new callbacks subcommand (text + JSON renderers + CLI wiring)
```

---

## Phase 0: Pre-Implementation Verification

Before starting Phase 2 (extraction tasks), verify the shared utility function that all extraction
tasks depend on exists and has the expected signature.

**Verify `spec::schema_name_from_ref` exists:**

```bash
grep -n "fn schema_name_from_ref" /home/hhewett/.local/src/phyllotaxis/src/spec.rs
```

Expected output: a function at `src/spec.rs` that takes a `&str` reference string and returns
`Option<&str>`. The function strips the `#/components/schemas/` prefix and returns the bare schema
name, or `None` for invalid refs.

**Confirmed:** `spec::schema_name_from_ref` exists in `src/spec.rs` (line 244) with signature:
```rust
pub fn schema_name_from_ref(reference: &str) -> Option<&str>
```

It returns `None` for empty strings, nested paths (containing `/`), and non-schema refs like
`#/definitions/Pet`. All tasks that call it handle the `Option` return correctly by using `?` or
`.and_then(...)`.

---

## Phase 1: Model Changes

### Task 1 — Extend `Field` with `write_only`, `deprecated`, `constraints`

**File:** `src/models/resource.rs`
**Depends on:** nothing

Add three new fields to the `Field` struct. All are zero-valued by default so all existing struct-literal construction in tests compiles unchanged.

```rust
// In src/models/resource.rs — replace the Field struct definition

#[derive(Debug, Clone, serde::Serialize)]
pub struct Field {
    pub name: String,
    pub type_display: String,
    pub required: bool,
    pub optional: bool,
    pub nullable: bool,
    pub read_only: bool,
    pub write_only: bool,       // NEW — Gap #5
    pub deprecated: bool,       // NEW — Gap #6
    pub description: Option<String>,
    pub enum_values: Vec<String>,
    pub constraints: Vec<String>, // NEW — Gap #4 — pre-formatted: "min:3", "max:32", "pattern:^[a-z]+$"
    pub default_value: Option<serde_json::Value>,
    pub example: Option<serde_json::Value>,
    pub nested_schema_name: Option<String>,
    pub nested_fields: Vec<Field>,
}
```

**Why this placement:** All three are properties of a single field in a schema, parallel to `read_only` and `nullable` which are already there. Keeping them together makes the struct self-documenting.

**Gotcha:** Every place in the codebase that constructs a `Field` struct literal must add the new fields. After adding Tasks 1–5, run `cargo check` to find all broken sites. The blast radius is larger than it looks: `Field`, `Endpoint`, `Response`, and `SchemaModel` literals will ALL break simultaneously — likely 15–20+ sites across test files, not just the two or three mentioned per task. Expect broken sites in `build_fields` in `commands/resources.rs`, multiple `Field { ... }` literals in `render/text.rs` tests (around lines ~878, ~893, ~938, ~945, ~959), `render/json.rs` test (~line 422), and every `Endpoint { ... }` and `SchemaModel { ... }` literal in tests. Fix all in one pass after Task 5 completes. Each `Field` site gets `write_only: false, deprecated: false, constraints: vec![]`.

**Verification:**
```bash
cargo check 2>&1 | grep "missing field"
```

---

### Task 2 — Add `ResponseHeader` model and `headers` field to `Response`

**File:** `src/models/resource.rs`
**Depends on:** nothing

```rust
// Add after ExternalDoc struct

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseHeader {
    pub name: String,
    pub type_display: String,
    pub description: Option<String>,
}
```

Then extend `Response`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct Response {
    pub status_code: String,
    pub description: String,
    pub schema_ref: Option<String>,
    pub example: Option<serde_json::Value>,
    pub headers: Vec<ResponseHeader>,   // NEW — Gap #2
}
```

All existing `Response { ... }` construction sites in tests need `headers: vec![]` added. Run `cargo check` to find them.

---

### Task 3 — Add `ResponseLink` model and `links` field to `Response`

**File:** `src/models/resource.rs`
**Depends on:** Task 2 (structurally follows from extending `Response`)

```rust
// Add after ResponseHeader struct

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseLink {
    pub name: String,
    pub operation_id: String,
    /// Pre-formatted "(param) = (expression)" strings
    pub parameters: Vec<String>,
    pub description: Option<String>,
    /// Ready-to-run phyllotaxis drill command
    pub drill_command: Option<String>,
}
```

Extend `Response` (building on Task 2's version):

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct Response {
    pub status_code: String,
    pub description: String,
    pub schema_ref: Option<String>,
    pub example: Option<serde_json::Value>,
    pub headers: Vec<ResponseHeader>,
    pub links: Vec<ResponseLink>,       // NEW — Gap #3
}
```

All `Response { ... }` construction sites also get `links: vec![]`.

**Why `drill_command` on the model:** The command string (`phyllotaxis resources users GET /users/{userId}`) requires knowing the operation's path and resource slug, which are more easily assembled at extraction time than at render time. The renderer just prints what's there.

---

### Task 4 — Add `CallbackResponse`, `CallbackOperation`, `CallbackEntry` models; add `callbacks` to `Endpoint`

**File:** `src/models/resource.rs`
**Depends on:** nothing

```rust
// Add after ResponseLink struct

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackResponse {
    pub status_code: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackOperation {
    pub method: String,
    /// The URL expression string, e.g. "{$request.query.callbackUrl}/events"
    pub url_expression: String,
    pub summary: Option<String>,
    /// Schema name if the body is a $ref, or "inline object" otherwise
    pub body_schema: Option<String>,
    pub responses: Vec<CallbackResponse>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackEntry {
    pub name: String,
    /// operationId and path of the endpoint that defines this callback
    pub defined_on_operation_id: Option<String>,
    pub defined_on_method: String,
    pub defined_on_path: String,
    pub operations: Vec<CallbackOperation>,
}
```

Add to `Endpoint`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct Endpoint {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub is_alpha: bool,
    pub external_docs: Option<ExternalDoc>,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: Vec<Response>,
    pub security_schemes: Vec<String>,
    pub callbacks: Vec<CallbackEntry>,   // NEW — Gap #3
    pub links: Vec<ResponseLink>,        // NEW — Gap #3 (aggregate of all response links)
    pub drill_deeper: Vec<String>,
}
```

**Design note on `links` duplication:** Links live on individual responses in the OAS spec. For convenience the renderer also wants a flat list of all links across all responses on the endpoint (for the text "Links:" section at the bottom). We store both: `Response.links` for the context-aware view, `Endpoint.links` as the flat aggregate. Extraction (Task 10) populates both from the same source data.

All `Endpoint { ... }` construction in tests need `callbacks: vec![], links: vec![]`.

**Important — derives required for JSON serialization:** All four new structs (`CallbackResponse`,
`CallbackOperation`, `CallbackEntry`, and the updated `Endpoint`) must have
`#[derive(Debug, Clone, serde::Serialize)]`. This is already shown in the code blocks above. The
JSON callback renderer in Task 22 uses `serialize(cb, is_tty)` which calls `serde_json::to_string`
directly on `CallbackEntry` — if any struct in the tree is missing `serde::Serialize`, the compile
will fail at that point rather than at model definition. Confirm all four structs carry the derive
before proceeding to Phase 4.

---

### Task 5 — Add `title` to `SchemaModel`

**File:** `src/models/schema.rs`
**Depends on:** nothing

```rust
#[derive(Debug, serde::Serialize)]
pub struct SchemaModel {
    pub name: String,
    pub title: Option<String>,          // NEW — Gap #7
    pub description: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub discriminator: Option<DiscriminatorInfo>,
    pub external_docs: Option<super::resource::ExternalDoc>,
}
```

All `SchemaModel { ... }` construction sites need `title: None`.

---

## Phase 2: Extraction Changes

**Note on kitchen-sink fixture:** `tests/fixtures/kitchen-sink.yaml` already exists — it was created prior to this plan. No creation task is needed before starting Phase 2.

**Shared test helper:** Tasks 6–13 all load the kitchen-sink fixture in their tests. Define
`load_kitchen_sink()` once as a module-level helper in the test module of `commands/resources.rs`
(and once in `commands/schemas.rs`, and once in `commands/callbacks.rs`). Do NOT copy-paste it into
each individual `#[test]` function — define it once per file at the top of the `#[cfg(test)] mod
tests` block and call it from all tests in that file.

```rust
// In #[cfg(test)] mod tests { ... } at the top of the block:
fn load_kitchen_sink() -> openapiv3::OpenAPI {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
    serde_yaml_ng::from_str(&content).unwrap()
}
```

### Task 6 — Extract `write_only` and `deprecated` into `build_fields`

**File:** `src/commands/resources.rs`
**Depends on:** Task 1

In `build_fields`, in the `fields.push(Field { ... })` call, add extraction of the two new bool fields from `SchemaData`:

```rust
fields.push(Field {
    name: name.clone(),
    type_display,
    required: required_fields.contains(name),
    optional: !required_fields.contains(name),
    nullable: resolved.schema_data.nullable,
    read_only: resolved.schema_data.read_only,
    write_only: resolved.schema_data.write_only,   // NEW
    deprecated: resolved.schema_data.deprecated,   // NEW
    description: resolved.schema_data.description.clone(),
    enum_values,
    constraints: vec![],   // placeholder; Task 7 fills this in
    default_value: resolved.schema_data.default.clone(),
    example: resolved.schema_data.example.clone(),
    nested_schema_name: schema_name.map(|s| s.to_string()),
    nested_fields: Vec::new(),
});
```

**How it works:** The `openapiv3` crate's `SchemaData` struct already has `write_only: bool` and `deprecated: bool`. The crate parses `writeOnly` and `deprecated` from YAML/JSON — we just weren't reading them.

**Test to write first (TDD):**

In `src/commands/resources.rs` tests:

```rust
fn load_kitchen_sink() -> openapiv3::OpenAPI {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
    serde_yaml_ng::from_str(&content).unwrap()
}

#[test]
fn test_write_only_field_extraction() {
    let api = load_kitchen_sink();
    let schema = api.components.as_ref().unwrap().schemas.get("CreateUserRequest").unwrap();
    let schema = match schema {
        openapiv3::ReferenceOr::Item(s) => s,
        _ => panic!("expected item"),
    };
    let fields = build_fields(&api, schema, &["username".to_string(), "email".to_string(), "password".to_string()]);
    let password = fields.iter().find(|f| f.name == "password").expect("password field");
    assert!(password.write_only, "password should be write_only");
}

#[test]
fn test_deprecated_field_extraction() {
    let api = load_kitchen_sink();
    let schema = api.components.as_ref().unwrap().schemas.get("PetBase").unwrap();
    let schema = match schema {
        openapiv3::ReferenceOr::Item(s) => s,
        _ => panic!("expected item"),
    };
    let fields = build_fields(&api, schema, &[]);
    let legacy = fields.iter().find(|f| f.name == "legacy_code").expect("legacy_code field");
    assert!(legacy.deprecated, "legacy_code should be deprecated");
}
```

Run `cargo test test_write_only_field_extraction` — expect failure. Implement. Run again — expect pass.

---

### Task 7 — Extract schema constraints into `build_fields`

**File:** `src/commands/resources.rs`
**Depends on:** Task 1, Task 6 (adds `constraints` field, Task 6 leaves it empty)

Add a helper function `extract_constraints` that builds the `Vec<String>` of pre-formatted constraint strings:

```rust
fn extract_constraints(kind: &openapiv3::SchemaKind) -> Vec<String> {
    let mut c = Vec::new();
    match kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => {
            if let Some(min) = s.min_length { c.push(format!("min:{}", min)); }
            if let Some(max) = s.max_length { c.push(format!("max:{}", max)); }
            if let Some(ref pat) = s.pattern   { c.push(format!("pattern:{}", pat)); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(i)) => {
            if let Some(ref min) = i.minimum { c.push(format!("min:{}", min)); }
            if let Some(ref max) = i.maximum { c.push(format!("max:{}", max)); }
            if let Some(ref mo)  = i.multiple_of { c.push(format!("multipleOf:{}", mo)); }
            if i.exclusive_minimum { c.push("exclusiveMinimum:true".to_string()); }
            if i.exclusive_maximum { c.push("exclusiveMaximum:true".to_string()); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(n)) => {
            if let Some(ref min) = n.minimum { c.push(format!("min:{}", min)); }
            if let Some(ref max) = n.maximum { c.push(format!("max:{}", max)); }
            if let Some(ref mo)  = n.multiple_of { c.push(format!("multipleOf:{}", mo)); }
            if n.exclusive_minimum { c.push("exclusiveMinimum:true".to_string()); }
            if n.exclusive_maximum { c.push("exclusiveMaximum:true".to_string()); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Array(a)) => {
            if let Some(min) = a.min_items { c.push(format!("minItems:{}", min)); }
            if let Some(max) = a.max_items { c.push(format!("maxItems:{}", max)); }
            if a.unique_items { c.push("uniqueItems:true".to_string()); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Object(o)) => {
            if let Some(min) = o.min_properties { c.push(format!("minProperties:{}", min)); }
            if let Some(max) = o.max_properties { c.push(format!("maxProperties:{}", max)); }
        }
        _ => {}
    }
    c
}
```

In `build_fields`, replace `constraints: vec![]` with `constraints: extract_constraints(&resolved.schema_kind)`.

**Note on openapiv3 types:** The `minimum`/`maximum` fields on `IntegerType` and `NumberType` are `Option<f64>` in openapiv3 v2.x. Confirm field names match:

```bash
cargo doc --open
# or inspect:
grep -r "exclusive_minimum\|exclusive_maximum\|min_length\|max_length" ~/.cargo/registry/src/ --include="*.rs" -l
```

If the crate uses different names (e.g., `exclusive_minimum` vs `exclusiveMinimum`), adjust accordingly. The openapiv3 crate maps YAML keys to snake_case Rust field names.

**Test to write first:**

```rust
#[test]
fn test_constraints_string_minlength_maxlength_pattern() {
    let api = load_kitchen_sink();
    // User.username has minLength:3, maxLength:32, pattern:'^[a-zA-Z0-9_-]+$'
    let schema = api.components.as_ref().unwrap().schemas.get("User").unwrap();
    let schema = match schema { openapiv3::ReferenceOr::Item(s) => s, _ => panic!() };
    let fields = build_fields(&api, schema, &[]);
    let username = fields.iter().find(|f| f.name == "username").expect("username");
    assert!(username.constraints.iter().any(|c| c.starts_with("min:")), "missing min: {:?}", username.constraints);
    assert!(username.constraints.iter().any(|c| c.starts_with("max:")), "missing max: {:?}", username.constraints);
    assert!(username.constraints.iter().any(|c| c.starts_with("pattern:")), "missing pattern: {:?}", username.constraints);
}

#[test]
fn test_constraints_integer() {
    let api = load_kitchen_sink();
    // Settings.max_upload_size_mb has minimum:1, maximum:1024, multipleOf:5
    let schema = api.components.as_ref().unwrap().schemas.get("Settings").unwrap();
    let schema = match schema { openapiv3::ReferenceOr::Item(s) => s, _ => panic!() };
    let fields = build_fields(&api, schema, &[]);
    let field = fields.iter().find(|f| f.name == "max_upload_size_mb").expect("max_upload_size_mb");
    assert!(field.constraints.iter().any(|c| c.starts_with("min:")), "missing min: {:?}", field.constraints);
    assert!(field.constraints.iter().any(|c| c.starts_with("max:")), "missing max: {:?}", field.constraints);
    assert!(field.constraints.iter().any(|c| c.starts_with("multipleOf:")), "missing multipleOf: {:?}", field.constraints);
}

#[test]
fn test_constraints_array_unique_items() {
    let api = load_kitchen_sink();
    // PetBase.tags has uniqueItems:true
    let schema = api.components.as_ref().unwrap().schemas.get("PetBase").unwrap();
    let schema = match schema { openapiv3::ReferenceOr::Item(s) => s, _ => panic!() };
    let fields = build_fields(&api, schema, &[]);
    let tags = fields.iter().find(|f| f.name == "tags").expect("tags");
    assert!(tags.constraints.iter().any(|c| c == "uniqueItems:true"), "missing uniqueItems: {:?}", tags.constraints);
}
```

---

### Task 8 — Fix integer/number enum extraction

**File:** `src/commands/resources.rs`
**Depends on:** Task 1

Current `extract_enum_values` only handles `Type::String`. Extend it to handle `Type::Integer` and `Type::Number`.

The openapiv3 crate stores integer enum values in `IntegerType.enumeration: Vec<Option<i64>>` and number enums in `NumberType.enumeration: Vec<Option<f64>>`.

Replace the existing `extract_enum_values` function:

```rust
fn extract_enum_values(kind: &openapiv3::SchemaKind) -> Vec<String> {
    match kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => s
            .enumeration
            .iter()
            .filter_map(|v| v.clone())
            .collect(),
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(i)) => i
            .enumeration
            .iter()
            .filter_map(|v| v.map(|n| n.to_string()))
            .collect(),
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(n)) => n
            .enumeration
            .iter()
            .filter_map(|v| v.map(|f| {
                // Format cleanly: 2.0 → "2", 2.5 → "2.5"
                if f.fract() == 0.0 { format!("{}", f as i64) } else { format!("{}", f) }
            }))
            .collect(),
        _ => Vec::new(),
    }
}
```

Also update `build_schema_model` in `commands/schemas.rs` to handle integer enums. Currently the `Composition::Enum` arm only matches `Type::String`. Add a parallel arm:

```rust
// In build_schema_model, in the schema_kind match:
openapiv3::SchemaKind::Type(openapiv3::Type::Integer(int_type))
    if !int_type.enumeration.is_empty() =>
{
    let values: Vec<String> = int_type
        .enumeration
        .iter()
        .filter_map(|v| v.map(|n| n.to_string()))
        .collect();
    (Vec::new(), Some(Composition::Enum(values)))
}
```

**Note on field names:** Verify `IntegerType` has an `.enumeration` field — openapiv3 v2.x should. Check with:

```bash
cargo doc -p openapiv3 2>/dev/null | grep -A5 "IntegerType"
```

**Test to write first:**

In `src/commands/schemas.rs` tests:

```rust
fn load_kitchen_sink_api() -> openapiv3::OpenAPI {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
    serde_yaml_ng::from_str(&content).unwrap()
}

#[test]
fn test_integer_enum_schema_model() {
    let api = load_kitchen_sink_api();
    // Priority is: type: integer, enum: [0, 1, 2, 3, 4]
    let model = build_schema_model(&api, "Priority", false, 5).unwrap();
    match &model.composition {
        Some(Composition::Enum(values)) => {
            assert!(values.contains(&"0".to_string()), "missing 0: {:?}", values);
            assert!(values.contains(&"4".to_string()), "missing 4: {:?}", values);
            assert_eq!(values.len(), 5);
        }
        other => panic!("Expected Enum, got {:?}", other),
    }
}
```

---

### Task 9 — Extract response headers in `extract_responses`

**File:** `src/commands/resources.rs`
**Depends on:** Task 2, Task 3

The openapiv3 crate's `Response` struct has a `headers: IndexMap<String, ReferenceOr<Header>>` field. Each `Header` has `description: Option<String>` and `format: ParameterSchemaOrContent` (not `data` — verified against openapiv3 v2.2.0).

Update `extract_responses` to populate the new `headers` vec:

```rust
fn extract_responses(operation: &openapiv3::Operation) -> Vec<crate::models::resource::Response> {
    use crate::models::resource::{Response, ResponseHeader};
    let mut responses = Vec::new();
    for (status, resp_ref) in &operation.responses.responses {
        let status_code = match status {
            openapiv3::StatusCode::Code(code) => code.to_string(),
            openapiv3::StatusCode::Range(range) => format!("{}XX", range),
        };

        let resp = match resp_ref {
            openapiv3::ReferenceOr::Item(r) => r,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        let (schema_ref_name, example) = resp
            .content
            .get("application/json")
            .map(|media| {
                let schema_ref = media.schema.as_ref().and_then(|sr| match sr {
                    openapiv3::ReferenceOr::Reference { reference } => {
                        spec::schema_name_from_ref(reference).map(|s| s.to_string())
                    }
                    _ => None,
                });
                (schema_ref, media.example.clone())
            })
            .unwrap_or((None, None));

        // NEW: extract response headers
        let headers: Vec<ResponseHeader> = resp
            .headers
            .iter()
            .filter_map(|(name, href)| {
                let header = match href {
                    openapiv3::ReferenceOr::Item(h) => h,
                    openapiv3::ReferenceOr::Reference { .. } => return None,
                };
                let type_display = match &header.format {
                    openapiv3::ParameterSchemaOrContent::Schema(s) => {
                        match s {
                            openapiv3::ReferenceOr::Item(schema) => {
                                format_type_display(&schema.schema_kind)
                            }
                            openapiv3::ReferenceOr::Reference { reference } => {
                                spec::schema_name_from_ref(reference)
                                    .unwrap_or("object")
                                    .to_string()
                            }
                        }
                    }
                    _ => "string".to_string(),
                };
                Some(ResponseHeader {
                    name: name.clone(),
                    type_display,
                    description: header.description.clone(),
                })
            })
            .collect();

        responses.push(Response {
            status_code,
            description: resp.description.clone(),
            schema_ref: schema_ref_name,
            example,
            headers,
            links: vec![], // Task 10 fills this in
        });
    }
    responses
}
```

**Note:** The `openapiv3::Header` struct may differ slightly between crate versions. Check the actual field name for `description` with `cargo doc`. If `Header` wraps a `ParameterData`, the description lives on `header.parameter_data.description`. Verify during implementation.

**Test to write first:**

```rust
#[test]
fn test_response_headers_extracted() {
    let api = load_kitchen_sink();
    // GET /users 200 response has X-Total-Count and X-Rate-Limit-Remaining headers
    let ep = get_endpoint_detail(&api, "GET", "/users", false).unwrap();
    let ok_resp = ep.responses.iter().find(|r| r.status_code == "200").unwrap();
    let header_names: Vec<&str> = ok_resp.headers.iter().map(|h| h.name.as_str()).collect();
    assert!(
        header_names.contains(&"X-Total-Count"),
        "missing X-Total-Count: {:?}", header_names
    );
    assert!(
        header_names.contains(&"X-Rate-Limit-Remaining"),
        "missing X-Rate-Limit-Remaining: {:?}", header_names
    );
}
```

---

### Task 10 — Extract links from responses in `get_endpoint_detail`

**File:** `src/commands/resources.rs`
**Depends on:** Task 3, Task 9

Links in OAS 3.0 live on response objects under `links: IndexMap<String, ReferenceOr<Link>>`. The `Link` struct has `operation: LinkOperation` (an enum, not a direct `operation_id` field), `parameters: IndexMap<String, serde_json::Value>`, and `description: Option<String>`. `LinkOperation` has two variants: `OperationId(String)` and `OperationRef(String)`.

**API note:** `link.operation_id` does NOT exist in openapiv3 v2.2.0. The correct access pattern is a match on `link.operation`.

First, update `extract_responses` (from Task 9) to also populate `links` on each `Response`. Then in `get_endpoint_detail`, aggregate all response links into a flat `Endpoint.links` vec for convenience.

Add a helper function:

```rust
fn extract_links_from_response(
    api: &openapiv3::OpenAPI,
    resp: &openapiv3::Response,
) -> Vec<crate::models::resource::ResponseLink> {
    use crate::models::resource::ResponseLink;

    resp.links
        .iter()
        .filter_map(|(link_name, link_ref)| {
            let link = match link_ref {
                openapiv3::ReferenceOr::Item(l) => l,
                openapiv3::ReferenceOr::Reference { .. } => return None,
            };

            let operation_id = match &link.operation {
                openapiv3::LinkOperation::OperationId(id) => id.clone(),
                openapiv3::LinkOperation::OperationRef(_) => return None, // skip ref-based links
            };

            // Build parameter display strings: "userId = $response.body#/id"
            let parameters: Vec<String> = link
                .parameters
                .iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    format!("{} = {}", k, val_str)
                })
                .collect();

            // Build drill command: look up the target operation's path + tag slug
            let drill_command = build_link_drill_command(api, &operation_id);

            Some(ResponseLink {
                name: link_name.clone(),
                operation_id,
                parameters,
                description: link.description.clone(),
                drill_command,
            })
        })
        .collect()
}

fn build_link_drill_command(
    api: &openapiv3::OpenAPI,
    operation_id: &str,
) -> Option<String> {
    // Find the path and method for this operationId
    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            _ => continue,
        };
        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get), ("POST", &path_item.post), ("PUT", &path_item.put),
            ("DELETE", &path_item.delete), ("PATCH", &path_item.patch),
        ];
        for &(method, op_opt) in methods {
            if let Some(op) = op_opt {
                if op.operation_id.as_deref() == Some(operation_id) {
                    // Find resource slug from first tag
                    let slug = op.tags.first().map(|t| {
                        crate::models::resource::slugify(t)
                    });
                    if let Some(slug) = slug {
                        return Some(format!(
                            "phyllotaxis resources {} {} {}",
                            slug, method, path_str
                        ));
                    }
                }
            }
        }
    }
    None
}
```

Update `extract_responses` to call `extract_links_from_response` (passing the raw openapiv3 response), storing results in `Response.links`.

In `get_endpoint_detail`, after building `responses`, aggregate links into `Endpoint.links`:

```rust
// After: let responses = extract_responses(operation);
let endpoint_links: Vec<crate::models::resource::ResponseLink> = responses
    .iter()
    .flat_map(|r| r.links.iter().cloned())
    .collect();
```

Pass `endpoint_links` to the `Endpoint { links: endpoint_links, ... }` construction.

**Test to write first:**

```rust
#[test]
fn test_links_extracted_from_post_users() {
    let api = load_kitchen_sink();
    // POST /users 201 response has GetCreatedUser and ListUserPets links
    let ep = get_endpoint_detail(&api, "POST", "/users", false).unwrap();
    // Check endpoint-level flat links
    let link_names: Vec<&str> = ep.links.iter().map(|l| l.name.as_str()).collect();
    assert!(link_names.contains(&"GetCreatedUser"), "missing GetCreatedUser: {:?}", link_names);
    assert!(link_names.contains(&"ListUserPets"), "missing ListUserPets: {:?}", link_names);
    // Check parameter mappings
    let get_user_link = ep.links.iter().find(|l| l.name == "GetCreatedUser").unwrap();
    assert!(!get_user_link.parameters.is_empty(), "GetCreatedUser should have parameter mappings");
    assert!(get_user_link.parameters.iter().any(|p| p.contains("userId")));
}

#[test]
fn test_link_drill_command_built() {
    let api = load_kitchen_sink();
    let ep = get_endpoint_detail(&api, "POST", "/users", false).unwrap();
    let link = ep.links.iter().find(|l| l.name == "GetCreatedUser").unwrap();
    // Should produce a phyllotaxis resources command for the target operation
    assert!(
        link.drill_command.as_ref().map(|c| c.contains("phyllotaxis resources")).unwrap_or(false),
        "Expected drill command, got: {:?}", link.drill_command
    );
}
```

---

### Task 11 — Extract callbacks inline on `Endpoint`

**File:** `src/commands/resources.rs`
**Depends on:** Task 4, Task 12 (the shared helper lives in `callbacks.rs` — implement Task 12 first)

**Implementation order note:** Task 12 defines the shared `extract_callbacks_from_operation` helper
in `callbacks.rs`. Task 11 is then just a thin wrapper in `resources.rs` that calls that helper.
Implement Task 12 before Task 11.

In `get_endpoint_detail`, after building responses, add:

```rust
let callbacks = crate::commands::callbacks::extract_callbacks_from_operation(operation, method, path);
```

Add `callbacks` to the returned `Endpoint`. No other code needed in `resources.rs` — all extraction
logic lives in `callbacks.rs`.

**Note on `openapiv3::Callback` type:** In openapiv3 v2.2.0, `Callback` is `IndexMap<String, PathItem>` — values are `PathItem` directly, NOT `ReferenceOr<PathItem>`. Verified against the crate source.

**Test to write first:**

```rust
#[test]
fn test_callbacks_extracted_inline() {
    let api = load_kitchen_sink();
    // POST /notifications/subscribe has onEvent and onStatusChange callbacks
    let ep = get_endpoint_detail(&api, "POST", "/notifications/subscribe", false).unwrap();
    let cb_names: Vec<&str> = ep.callbacks.iter().map(|c| c.name.as_str()).collect();
    assert!(cb_names.contains(&"onEvent"), "missing onEvent: {:?}", cb_names);
    assert!(cb_names.contains(&"onStatusChange"), "missing onStatusChange: {:?}", cb_names);

    let on_event = ep.callbacks.iter().find(|c| c.name == "onEvent").unwrap();
    assert!(!on_event.operations.is_empty(), "onEvent should have operations");
    let op = &on_event.operations[0];
    assert_eq!(op.method, "POST");
    assert!(op.url_expression.contains("callbackUrl"), "URL expression: {}", op.url_expression);
    assert_eq!(op.body_schema.as_deref(), Some("EventPayload"),
        "onEvent body should be EventPayload, got {:?}", op.body_schema);
}

#[test]
fn test_callback_responses_extracted() {
    let api = load_kitchen_sink();
    let ep = get_endpoint_detail(&api, "POST", "/notifications/subscribe", false).unwrap();
    let on_event = ep.callbacks.iter().find(|c| c.name == "onEvent").unwrap();
    let op = &on_event.operations[0];
    let status_codes: Vec<&str> = op.responses.iter().map(|r| r.status_code.as_str()).collect();
    assert!(status_codes.contains(&"200"), "missing 200: {:?}", status_codes);
    assert!(status_codes.contains(&"410"), "missing 410: {:?}", status_codes);
}
```

---

### Task 12 — New callbacks extraction module (implement before Task 11)

**File:** `src/commands/callbacks.rs` (new file)
**Depends on:** Task 4

Create a new module that owns ALL callback extraction logic. Both Task 11 (inline extraction on
`Endpoint`) and the global `list_all_callbacks` function use the same shared
`extract_callbacks_from_operation` helper defined here. This means the extraction logic is defined
exactly once.

```rust
// src/commands/callbacks.rs

use crate::models::resource::{CallbackEntry, CallbackOperation, CallbackResponse};

/// Build a CallbackEntry for a single named callback on a single operation.
/// Returns None if the callback has no recognizable operations.
///
/// This is the shared helper used by both:
///   - resources.rs (extract_callbacks_from_operation for inline Endpoint.callbacks)
///   - list_all_callbacks (global scan)
pub fn extract_callbacks_from_operation(
    operation: &openapiv3::Operation,
    method: &str,
    path: &str,
) -> Vec<CallbackEntry> {
    use crate::spec;

    // operation.callbacks is IndexMap<String, Callback> — no ReferenceOr wrapper here.
    operation
        .callbacks
        .iter()
        .filter_map(|(callback_name, callback)| {
            build_callback_entry(callback_name, callback, operation, method, path)
        })
        .collect()
}

fn build_callback_entry(
    callback_name: &str,
    callback: &openapiv3::Callback,
    operation: &openapiv3::Operation,
    method: &str,
    path: &str,
) -> Option<CallbackEntry> {
    use crate::spec;

    let operations: Vec<CallbackOperation> = callback
        .iter()
        .flat_map(|(url_expr, path_item)| {
            // path_item is &PathItem directly — Callback is IndexMap<String, PathItem>,
            // not IndexMap<String, ReferenceOr<PathItem>>.

            // Collect all HTTP methods defined on the callback path item
            let cb_methods: &[(&str, &Option<openapiv3::Operation>)] = &[
                ("POST", &path_item.post), ("GET", &path_item.get),
                ("PUT", &path_item.put), ("DELETE", &path_item.delete),
                ("PATCH", &path_item.patch),
            ];
            cb_methods.iter().filter_map(|&(m, op_opt)| {
                let op = op_opt.as_ref()?;

                // Determine body schema name
                let body_schema = op.request_body.as_ref().and_then(|rb_ref| {
                    let rb = match rb_ref {
                        openapiv3::ReferenceOr::Item(rb) => rb,
                        _ => return None,
                    };
                    // Try application/json first, then any content type
                    let media = rb.content.get("application/json")
                        .or_else(|| rb.content.values().next())?;
                    match media.schema.as_ref()? {
                        openapiv3::ReferenceOr::Reference { reference } => {
                            spec::schema_name_from_ref(reference).map(|s| s.to_string())
                        }
                        openapiv3::ReferenceOr::Item(_) => {
                            Some("inline object".to_string())
                        }
                    }
                });

                // Extract responses — iterate once, use the ref directly (no double-fetch)
                let responses: Vec<CallbackResponse> = op
                    .responses
                    .responses
                    .iter()
                    .map(|(status, resp_ref)| {
                        let code = match status {
                            openapiv3::StatusCode::Code(c) => c.to_string(),
                            openapiv3::StatusCode::Range(r) => format!("{}XX", r),
                        };
                        let desc = match resp_ref {
                            openapiv3::ReferenceOr::Item(r) => r.description.clone(),
                            _ => String::new(),
                        };
                        CallbackResponse { status_code: code, description: desc }
                    })
                    .collect();

                Some(CallbackOperation {
                    method: m.to_string(),
                    url_expression: url_expr.clone(),
                    summary: op.summary.clone(),
                    body_schema,
                    responses,
                })
            }).collect::<Vec<_>>()
        })
        .collect();

    if operations.is_empty() {
        return None;
    }

    Some(CallbackEntry {
        name: callback_name.to_string(),
        defined_on_operation_id: operation.operation_id.clone(),
        defined_on_method: method.to_uppercase(),
        defined_on_path: path.to_string(),
        operations,
    })
}

/// Returns all callbacks defined anywhere in the spec, one entry per callback name per operation.
pub fn list_all_callbacks(api: &openapiv3::OpenAPI) -> Vec<CallbackEntry> {
    let mut entries: Vec<CallbackEntry> = Vec::new();

    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            _ => continue,
        };

        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get), ("POST", &path_item.post), ("PUT", &path_item.put),
            ("DELETE", &path_item.delete), ("PATCH", &path_item.patch),
            ("HEAD", &path_item.head), ("OPTIONS", &path_item.options), ("TRACE", &path_item.trace),
        ];

        for &(method, op_opt) in methods {
            let op = match op_opt {
                Some(op) => op,
                None => continue,
            };

            let mut found = extract_callbacks_from_operation(op, method, path_str);
            entries.append(&mut found);
        }
    }

    entries
}

/// Find a specific callback by name across all operations.
/// Returns None if not found.
/// Note: callback name matching is case-sensitive.
pub fn find_callback(api: &openapiv3::OpenAPI, name: &str) -> Option<CallbackEntry> {
    list_all_callbacks(api)
        .into_iter()
        .find(|e| e.name == name)
}
```

**Register the new module:** In `src/commands/mod.rs` (or wherever commands are declared), add:

```rust
pub mod callbacks;
```

Check the existing module declaration pattern first:

```bash
cat /home/hhewett/.local/src/phyllotaxis/src/commands/mod.rs
# or check lib.rs if commands are declared there
```

**Test to write first:**

In `src/commands/callbacks.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn load_kitchen_sink() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_list_all_callbacks_finds_on_event() {
        let api = load_kitchen_sink();
        let callbacks = list_all_callbacks(&api);
        let names: Vec<&str> = callbacks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"onEvent"), "missing onEvent: {:?}", names);
        assert!(names.contains(&"onStatusChange"), "missing onStatusChange: {:?}", names);
    }

    #[test]
    fn test_find_callback_by_name() {
        let api = load_kitchen_sink();
        let cb = find_callback(&api, "onEvent");
        assert!(cb.is_some(), "onEvent should be findable");
        let cb = cb.unwrap();
        assert_eq!(cb.defined_on_method, "POST");
        assert_eq!(cb.defined_on_path, "/notifications/subscribe");
    }

    #[test]
    fn test_find_callback_not_found() {
        let api = load_kitchen_sink();
        let cb = find_callback(&api, "nonexistent");
        assert!(cb.is_none());
    }
}
```

---

### Task 13 — Extract `title` in `build_schema_model`

**File:** `src/commands/schemas.rs`
**Depends on:** Task 5

The `openapiv3::SchemaData` struct has a `title: Option<String>` field.

In `build_schema_model`, extract it alongside `description`:

```rust
let description = schema.schema_data.description.clone();
let title = schema.schema_data.title.clone();  // NEW
```

Pass `title` to the returned `SchemaModel`:

```rust
Some(SchemaModel {
    name: name.to_string(),
    title,              // NEW
    description,
    fields,
    composition,
    discriminator,
    external_docs: None,
})
```

**Test to write first:**

```rust
#[test]
fn test_schema_title_extracted() {
    let api = load_kitchen_sink_api();
    // GeoLocation has title: "Geographic Location"
    let model = build_schema_model(&api, "GeoLocation", false, 5).unwrap();
    assert_eq!(
        model.title.as_deref(),
        Some("Geographic Location"),
        "GeoLocation should have title 'Geographic Location', got: {:?}", model.title
    );
}

#[test]
fn test_schema_no_title_is_none() {
    let api = load_kitchen_sink_api();
    let model = build_schema_model(&api, "User", false, 5).unwrap();
    assert!(model.title.is_none(), "User has no title, got: {:?}", model.title);
}
```

---

## Phase 3: Request Body Overhaul

### Task 14 — Multi-content-type request body extraction (Gap #1)

**File:** `src/commands/resources.rs`
**Depends on:** Tasks 6, 7, 8 (Field struct must have constraints/write_only/deprecated; `build_fields` must be updated)

The current `extract_request_body` hard-codes `.get("application/json")`. Replace with iteration over all content types, prioritizing JSON but falling back to multipart and form-urlencoded.

Also update `RequestBody` in `models/resource.rs` to drop the single `content_type: String` in favor of `content_types: Vec<String>` — or keep `content_type` as the "first/primary" type. The design doc says "store content type on RequestBody struct for display" and the existing struct already has this. The key change is which content type we look at.

**Strategy:** Iterate content entries in priority order: `application/json` first, then `multipart/form-data`, then `application/x-www-form-urlencoded`, then any other. Use the first one that has a schema. Store its content-type key.

Replace `extract_request_body`:

```rust
fn extract_request_body(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
    expand: bool,
) -> Option<crate::models::resource::RequestBody> {
    use crate::models::resource::RequestBody;

    let rb_ref = operation.request_body.as_ref()?;
    let rb = match rb_ref {
        openapiv3::ReferenceOr::Item(rb) => rb,
        openapiv3::ReferenceOr::Reference { .. } => return None,
    };

    // Priority order: JSON first (existing behavior), then multipart, then form-encoded, then any
    let priority = [
        "application/json",
        "multipart/form-data",
        "application/x-www-form-urlencoded",
    ];

    let (content_type, media) = priority
        .iter()
        .find_map(|ct| rb.content.get(*ct).map(|m| (*ct, m)))
        .or_else(|| rb.content.iter().next().map(|(ct, m)| (ct.as_str(), m)))?;

    let schema_ref = media.schema.as_ref()?;

    let (schema, top_ref_name): (&openapiv3::Schema, Option<String>) = match schema_ref {
        openapiv3::ReferenceOr::Item(s) => (s, None),
        openapiv3::ReferenceOr::Reference { reference } => {
            let sname = spec::schema_name_from_ref(reference)?;
            let components = api.components.as_ref()?;
            match components.schemas.get(sname)? {
                openapiv3::ReferenceOr::Item(s) => (s, Some(sname.to_string())),
                _ => return None,
            }
        }
    };

    let example = media.example.clone();

    // OneOf/AnyOf request body
    let oneof_variants: &[openapiv3::ReferenceOr<openapiv3::Schema>] = match &schema.schema_kind {
        openapiv3::SchemaKind::OneOf { one_of } => one_of.as_slice(),
        openapiv3::SchemaKind::AnyOf { any_of } => any_of.as_slice(),
        _ => &[],
    };
    let options: Vec<String> = oneof_variants
        .iter()
        .filter_map(|r| match r {
            openapiv3::ReferenceOr::Reference { reference } => {
                spec::schema_name_from_ref(reference).map(|s| s.to_string())
            }
            _ => None,
        })
        .collect();

    if !options.is_empty() {
        return Some(RequestBody {
            content_type: content_type.to_string(),
            fields: Vec::new(),
            options,
            schema_ref: None,
            example,
        });
    }

    let required: Vec<String> =
        if let openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) = &schema.schema_kind {
            obj.required.clone()
        } else {
            vec![]
        };

    let mut fields = build_fields(api, schema, &required);

    if expand {
        use crate::commands::schemas::expand_fields_pub;
        let mut visited = std::collections::HashSet::new();
        fields = expand_fields_pub(api, fields, &mut visited, 1, 5);
    }

    Some(RequestBody {
        content_type: content_type.to_string(),
        fields,
        options: Vec::new(),
        schema_ref: top_ref_name,
        example,
    })
}
```

**Binary field type display:** The design doc says binary fields display as type `"binary"`. The
existing `format_type_display` function produces `"string/binary"` for `type: string, format:
binary`. The correct behavior is to produce `"binary"` — meaning the implementation must add a
special case in `format_type_display` (or in `build_fields`) to strip the `"string/"` prefix when
the format is `Binary`.

Add this override in `format_type_display` within the `String` arm, before the existing match on `s.format`:

```rust
openapiv3::Type::String(s) => {
    match &s.format {
        openapiv3::VariantOrUnknownOrEmpty::Item(fmt) => {
            // Binary is displayed as just "binary", not "string/binary"
            if matches!(fmt, openapiv3::StringFormat::Binary) {
                return "binary".to_string();
            }
            format!("string/{}", format_variant_name(fmt))
        }
        // ... rest unchanged
    }
}
```

**Scope decision — multipart per-field encoding info:** The design doc (Gap #1) listed "extract
encoding info (contentType overrides per field)" as a requirement. This feature is **descoped from
this implementation**. Reasons: encoding overrides affect how individual multipart fields are
transmitted on the wire — they don't change the field's logical type as seen in the schema (a `file`
field is still `type: string, format: binary` regardless of encoding override). For phyllotaxis's
goal of showing what fields an API expects, schema-level type info is sufficient. If per-field
encoding display is needed in a future iteration, add `encoding: Option<String>` to `Field` and
extract from `media.encoding`. Leave a comment at the extraction site:

```rust
// NOTE: multipart/form-data per-field encoding overrides (media.encoding) are intentionally
// not extracted. They describe wire-level content type, not the field's logical schema type.
```

**Test to write first:**

```rust
#[test]
fn test_multipart_request_body_extracted() {
    let api = load_kitchen_sink();
    // POST /files/upload has multipart/form-data with file (binary), description (string), tags (array)
    let ep = get_endpoint_detail(&api, "POST", "/files/upload", false).unwrap();
    let body = ep.request_body.as_ref().expect("should have request body");
    assert_eq!(body.content_type, "multipart/form-data");
    let field_names: Vec<&str> = body.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"file"), "missing file field: {:?}", field_names);
    assert!(field_names.contains(&"description"), "missing description: {:?}", field_names);
    assert!(field_names.contains(&"tags"), "missing tags: {:?}", field_names);
}

#[test]
fn test_multipart_binary_field_type() {
    let api = load_kitchen_sink();
    let ep = get_endpoint_detail(&api, "POST", "/files/upload", false).unwrap();
    let body = ep.request_body.as_ref().unwrap();
    let file_field = body.fields.iter().find(|f| f.name == "file").unwrap();
    // Design requirement: binary fields show as "binary", not "string/binary"
    assert_eq!(file_field.type_display, "binary", "file should be binary, got: {}", file_field.type_display);
}

#[test]
fn test_form_urlencoded_request_body() {
    let api = load_kitchen_sink();
    // PUT /files/{fileId}/metadata has application/x-www-form-urlencoded
    let ep = get_endpoint_detail(&api, "PUT", "/files/{fileId}/metadata", false).unwrap();
    let body = ep.request_body.as_ref().expect("should have request body");
    assert_eq!(body.content_type, "application/x-www-form-urlencoded");
    let field_names: Vec<&str> = body.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"description"), "missing description: {:?}", field_names);
}

#[test]
fn test_json_body_still_works_after_refactor() {
    // Regression: existing JSON body extraction should be unaffected
    let api = load_kitchen_sink();
    let ep = get_endpoint_detail(&api, "POST", "/users", false).unwrap();
    let body = ep.request_body.as_ref().expect("should have request body");
    assert_eq!(body.content_type, "application/json");
    assert!(body.schema_ref.is_some(), "JSON body should still have schema_ref");
}
```

---

## Phase 4: Rendering

### Task 15 — Text renderer: `write_only`, `deprecated`, `constraints`, integer enums in fields

**File:** `src/render/text.rs`
**Depends on:** Tasks 6, 7, 8

**In `render_fields_section` and `render_schema_fields`**, update the flags vector to include the new annotations, add constraints output, and handle non-string enum values (which are already stored as `Vec<String>` via Task 8, so no special enum rendering change is needed — they just display).

**Pattern for `write_only` and `deprecated`:**

```rust
// In both render_fields_section and render_schema_fields, add to flags:
if f.write_only {
    flags.push("write-only");
}
if f.deprecated {
    flags.push("DEPRECATED");  // all-caps as the design doc specifies [DEPRECATED]
}
```

**Pattern for constraints** (append inline after the description, before enum):

In the `writeln!` format call, add a constraints segment:

```rust
let constraints_str = if f.constraints.is_empty() {
    String::new()
} else {
    format!("  {}", f.constraints.join(" "))
};

// Then in the writeln! format string, append constraints_str after desc and before enums
writeln!(
    out,
    "  {:<nw$}  {:<tw$}  {:<20}  {}{}{}",
    sanitize(&f.name),
    sanitize(&f.type_display),
    flag_str,
    desc,
    constraints_str,
    enums,
    nw = max_name,
    tw = max_type,
).unwrap();
```

**Test to write first:**

In `src/render/text.rs` tests:

```rust
#[test]
fn test_render_write_only_flag() {
    use crate::models::resource::Field;

    let fields = vec![Field {
        name: "password".to_string(),
        type_display: "string".to_string(),
        required: true,
        optional: false,
        nullable: false,
        read_only: false,
        write_only: true,
        deprecated: false,
        description: None,
        enum_values: vec![],
        constraints: vec![],
        default_value: None,
        example: None,
        nested_schema_name: None,
        nested_fields: vec![],
    }];

    let mut out = String::new();
    // Call render_fields_section (needs to be pub(crate) or tested via render_endpoint_detail)
    // Use render_schema_detail as the vehicle:
    use crate::models::schema::SchemaModel;
    let model = SchemaModel {
        name: "Test".to_string(),
        title: None,
        description: None,
        fields,
        composition: None,
        discriminator: None,
        external_docs: None,
    };
    let output = render_schema_detail(&model, false, false);
    assert!(output.contains("write-only"), "Missing write-only flag, got:\n{}", output);
}

#[test]
fn test_render_deprecated_field_flag() {
    use crate::models::resource::Field;
    use crate::models::schema::SchemaModel;

    let model = SchemaModel {
        name: "Test".to_string(),
        title: None,
        description: None,
        fields: vec![Field {
            name: "legacy_code".to_string(),
            type_display: "string".to_string(),
            required: false,
            optional: true,
            nullable: false,
            read_only: false,
            write_only: false,
            deprecated: true,
            description: None,
            enum_values: vec![],
            constraints: vec![],
            default_value: None,
            example: None,
            nested_schema_name: None,
            nested_fields: vec![],
        }],
        composition: None,
        discriminator: None,
        external_docs: None,
    };
    let output = render_schema_detail(&model, false, false);
    assert!(output.contains("DEPRECATED"), "Missing DEPRECATED flag, got:\n{}", output);
}

#[test]
fn test_render_constraints_inline() {
    use crate::models::resource::Field;
    use crate::models::schema::SchemaModel;

    let model = SchemaModel {
        name: "Test".to_string(),
        title: None,
        description: None,
        fields: vec![Field {
            name: "username".to_string(),
            type_display: "string".to_string(),
            required: true,
            optional: false,
            nullable: false,
            read_only: false,
            write_only: false,
            deprecated: false,
            description: Some("Unique username".to_string()),
            enum_values: vec![],
            constraints: vec!["min:3".to_string(), "max:32".to_string(), "pattern:^[a-zA-Z0-9_-]+$".to_string()],
            default_value: None,
            example: None,
            nested_schema_name: None,
            nested_fields: vec![],
        }],
        composition: None,
        discriminator: None,
        external_docs: None,
    };
    let output = render_schema_detail(&model, false, false);
    assert!(output.contains("min:3"), "Missing min:3, got:\n{}", output);
    assert!(output.contains("max:32"), "Missing max:32, got:\n{}", output);
    assert!(output.contains("pattern:"), "Missing pattern, got:\n{}", output);
}

#[test]
fn test_render_integer_enum() {
    use crate::models::resource::Field;
    use crate::models::schema::SchemaModel;

    let model = SchemaModel {
        name: "Priority".to_string(),
        title: None,
        description: None,
        fields: vec![],
        composition: Some(crate::models::schema::Composition::Enum(
            vec!["0".to_string(), "1".to_string(), "2".to_string()]
        )),
        discriminator: None,
        external_docs: None,
    };
    let output = render_schema_detail(&model, false, false);
    assert!(output.contains("0"), "Missing integer 0 in enum, got:\n{}", output);
    assert!(output.contains("1"), "Missing integer 1 in enum, got:\n{}", output);
}
```

---

### Task 16 — Text renderer: response headers

**File:** `src/render/text.rs`
**Depends on:** Task 4, Task 9

In `render_endpoint_detail`, after printing each response line in the Responses section, if the response has headers, print them:

```rust
// After the response line in the successes loop:
if !resp.headers.is_empty() {
    out.push_str("    Headers:\n");
    for h in &resp.headers {
        let desc = sanitize(h.description.as_deref().unwrap_or(""));
        writeln!(out, "      {}  {}  {}", sanitize(&h.name), sanitize(&h.type_display), desc).unwrap();
    }
}
```

**Test to write first:**

```rust
#[test]
fn test_render_response_headers() {
    use crate::models::resource::*;

    let endpoint = Endpoint {
        method: "GET".to_string(),
        path: "/users".to_string(),
        summary: None,
        description: None,
        is_deprecated: false,
        is_alpha: false,
        external_docs: None,
        parameters: vec![],
        request_body: None,
        responses: vec![Response {
            status_code: "200".to_string(),
            description: "OK".to_string(),
            schema_ref: None,
            example: None,
            headers: vec![
                ResponseHeader {
                    name: "X-Total-Count".to_string(),
                    type_display: "integer".to_string(),
                    description: Some("Total count".to_string()),
                },
            ],
            links: vec![],
        }],
        security_schemes: vec![],
        callbacks: vec![],
        links: vec![],
        drill_deeper: vec![],
    };

    let output = render_endpoint_detail(&endpoint, false);
    assert!(output.contains("X-Total-Count"), "Missing header name, got:\n{}", output);
    assert!(output.contains("integer"), "Missing header type, got:\n{}", output);
}
```

---

### Task 17 — Text renderer: links section

**File:** `src/render/text.rs`
**Depends on:** Task 4, Task 10

In `render_endpoint_detail`, after the Errors section and before Drill deeper, add a Links section:

```rust
if !endpoint.links.is_empty() {
    out.push_str("\nLinks:\n");
    for link in &endpoint.links {
        writeln!(out, "  {} -> {}", sanitize(&link.name), sanitize(&link.operation_id)).unwrap();
        if let Some(ref desc) = link.description {
            writeln!(out, "    {}", sanitize(desc)).unwrap();
        }
        for param in &link.parameters {
            writeln!(out, "    {}", sanitize(param)).unwrap();
        }
        if let Some(ref cmd) = link.drill_command {
            writeln!(out, "    {}", sanitize(cmd)).unwrap();
        }
    }
}
```

**Test to write first:**

```rust
#[test]
fn test_render_links_section() {
    use crate::models::resource::*;

    let endpoint = Endpoint {
        method: "POST".to_string(),
        path: "/users".to_string(),
        summary: None,
        description: None,
        is_deprecated: false,
        is_alpha: false,
        external_docs: None,
        parameters: vec![],
        request_body: None,
        responses: vec![Response {
            status_code: "201".to_string(),
            description: "Created".to_string(),
            schema_ref: None,
            example: None,
            headers: vec![],
            links: vec![ResponseLink {
                name: "GetCreatedUser".to_string(),
                operation_id: "getUser".to_string(),
                parameters: vec!["userId = $response.body#/id".to_string()],
                description: None,
                drill_command: Some("phyllotaxis resources users GET /users/{userId}".to_string()),
            }],
        }],
        security_schemes: vec![],
        callbacks: vec![],
        links: vec![ResponseLink {
            name: "GetCreatedUser".to_string(),
            operation_id: "getUser".to_string(),
            parameters: vec!["userId = $response.body#/id".to_string()],
            description: None,
            drill_command: Some("phyllotaxis resources users GET /users/{userId}".to_string()),
        }],
        drill_deeper: vec![],
    };

    let output = render_endpoint_detail(&endpoint, false);
    assert!(output.contains("Links:"), "Missing Links section, got:\n{}", output);
    assert!(output.contains("GetCreatedUser"), "Missing link name, got:\n{}", output);
    assert!(output.contains("getUser"), "Missing operationId, got:\n{}", output);
    assert!(output.contains("userId = $response.body#/id"), "Missing parameter mapping, got:\n{}", output);
}
```

---

### Task 18 — Text renderer: callbacks inline section

**File:** `src/render/text.rs`
**Depends on:** Task 4, Task 11

In `render_endpoint_detail`, after the Links section:

```rust
if !endpoint.callbacks.is_empty() {
    out.push_str("\nCallbacks:\n");
    for cb in &endpoint.callbacks {
        for op in &cb.operations {
            writeln!(
                out,
                "  {} -> {} {}",
                sanitize(&cb.name),
                sanitize(&op.method),
                sanitize(&op.url_expression)
            ).unwrap();
            if let Some(ref schema) = op.body_schema {
                writeln!(out, "    Body: {}", sanitize(schema)).unwrap();
            }
            if !op.responses.is_empty() {
                let codes: Vec<String> = op.responses.iter()
                    .map(|r| sanitize(&r.status_code))
                    .collect();
                writeln!(out, "    Responses: {}", codes.join(", ")).unwrap();
            }
        }
    }
}
```

**Test to write first:**

```rust
#[test]
fn test_render_callbacks_inline() {
    use crate::models::resource::*;

    let endpoint = Endpoint {
        method: "POST".to_string(),
        path: "/notifications/subscribe".to_string(),
        summary: None,
        description: None,
        is_deprecated: false,
        is_alpha: false,
        external_docs: None,
        parameters: vec![],
        request_body: None,
        responses: vec![],
        security_schemes: vec![],
        callbacks: vec![CallbackEntry {
            name: "onEvent".to_string(),
            defined_on_operation_id: Some("subscribeNotifications".to_string()),
            defined_on_method: "POST".to_string(),
            defined_on_path: "/notifications/subscribe".to_string(),
            operations: vec![CallbackOperation {
                method: "POST".to_string(),
                url_expression: "{$request.query.callbackUrl}/events".to_string(),
                summary: Some("Event notification callback".to_string()),
                body_schema: Some("EventPayload".to_string()),
                responses: vec![
                    CallbackResponse { status_code: "200".to_string(), description: "Acknowledged".to_string() },
                ],
            }],
        }],
        links: vec![],
        drill_deeper: vec![],
    };

    let output = render_endpoint_detail(&endpoint, false);
    assert!(output.contains("Callbacks:"), "Missing Callbacks section, got:\n{}", output);
    assert!(output.contains("onEvent"), "Missing callback name, got:\n{}", output);
    assert!(output.contains("EventPayload"), "Missing body schema, got:\n{}", output);
    assert!(output.contains("{$request.query.callbackUrl}/events"), "Missing URL expression, got:\n{}", output);
}
```

---

### Task 19 — Text renderer: schema title display

**File:** `src/render/text.rs`
**Depends on:** Task 13

In `render_schema_detail`, update the header line:

```rust
// Replace the existing header block:
if expanded {
    writeln!(out, "Schema: {} (expanded)", sanitize(&model.name)).unwrap();
} else {
    writeln!(out, "Schema: {}", sanitize(&model.name)).unwrap();
}

// NEW: show title if different from name
if let Some(ref title) = model.title {
    if title != &model.name {
        writeln!(out, "Title: {}", sanitize(title)).unwrap();
    }
}
```

**Test to write first:**

```rust
#[test]
fn test_render_schema_title_shown_when_different() {
    use crate::models::schema::SchemaModel;

    let model = SchemaModel {
        name: "GeoLocation".to_string(),
        title: Some("Geographic Location".to_string()),
        description: Some("GPS coordinates".to_string()),
        fields: vec![],
        composition: None,
        discriminator: None,
        external_docs: None,
    };

    let output = render_schema_detail(&model, false, false);
    assert!(output.contains("Schema: GeoLocation"), "Missing schema name, got:\n{}", output);
    assert!(output.contains("Geographic Location"), "Missing title, got:\n{}", output);
}

#[test]
fn test_render_schema_title_hidden_when_same_as_name() {
    use crate::models::schema::SchemaModel;

    let model = SchemaModel {
        name: "User".to_string(),
        title: Some("User".to_string()),  // same as name
        description: None,
        fields: vec![],
        composition: None,
        discriminator: None,
        external_docs: None,
    };

    let output = render_schema_detail(&model, false, false);
    // Should not print the title line if it's identical to the name
    let title_count = output.matches("User").count();
    // "Schema: User" appears once; "Title: User" should NOT appear
    assert!(!output.contains("Title:"), "Title should be hidden when same as name, got:\n{}", output);
}
```

---

### Task 20 — JSON renderer: update `FieldJson` and `convert_fields`

**File:** `src/render/json.rs`
**Depends on:** Tasks 6, 7, 8

**Ordering note:** Tasks 20 and 21 both extend `test_all_json_outputs_parse`. Task 20 must be applied before Task 21.

Add the new fields to `FieldJson`:

```rust
#[derive(serde::Serialize)]
struct FieldJson<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    type_display: &'a str,
    required: bool,
    optional: bool,
    nullable: bool,
    read_only: bool,
    write_only: bool,         // NEW
    deprecated: bool,         // NEW
    description: Option<&'a str>,
    enum_values: &'a [String],
    constraints: &'a [String], // NEW
    default: Option<&'a serde_json::Value>,
    nested_schema: Option<&'a str>,
    nested_fields: Vec<FieldJson<'a>>,
}
```

Update `convert_fields`:

```rust
fn convert_fields<'a>(fields: &'a [crate::models::resource::Field]) -> Vec<FieldJson<'a>> {
    fields
        .iter()
        .map(|f| FieldJson {
            name: &f.name,
            type_display: &f.type_display,
            required: f.required,
            optional: f.optional,
            nullable: f.nullable,
            read_only: f.read_only,
            write_only: f.write_only,     // NEW
            deprecated: f.deprecated,     // NEW
            description: f.description.as_deref(),
            enum_values: &f.enum_values,
            constraints: &f.constraints,  // NEW
            default: f.default_value.as_ref(),
            nested_schema: f.nested_schema_name.as_deref(),
            nested_fields: convert_fields(&f.nested_fields),
        })
        .collect()
}
```

Also update `render_schema_detail` to include `title` in `SchemaDetailJson`:

```rust
#[derive(serde::Serialize)]
struct SchemaDetailJson<'a> {
    name: &'a str,
    title: Option<&'a str>,    // NEW
    description: Option<&'a str>,
    // ... rest unchanged
}

// In the render_schema_detail function:
let json = SchemaDetailJson {
    name: &model.name,
    title: model.title.as_deref(),   // NEW
    description: model.description.as_deref(),
    // ...
};
```

**Test to write first:**

Extend the existing `test_all_json_outputs_parse` test to verify the new fields:

```rust
// In test_all_json_outputs_parse, after the endpoint detail section, add:
// Schema with title
let model_with_title = SchemaModel {
    name: "GeoLocation".to_string(),
    title: Some("Geographic Location".to_string()),
    description: None,
    fields: vec![],
    composition: None,
    discriminator: None,
    external_docs: None,
};
let v = parse_json(&render_schema_detail(&model_with_title, false));
assert_eq!(v["title"], "Geographic Location", "JSON should include title");

// Field with new properties
let endpoint_with_new_fields = Endpoint {
    // ... minimal valid endpoint with a field that has write_only, deprecated, constraints
    request_body: Some(RequestBody {
        content_type: "application/json".to_string(),
        fields: vec![Field {
            name: "password".to_string(),
            type_display: "string".to_string(),
            required: true,
            optional: false,
            nullable: false,
            read_only: false,
            write_only: true,
            deprecated: false,
            description: None,
            enum_values: vec![],
            constraints: vec!["min:8".to_string()],
            default_value: None,
            example: None,
            nested_schema_name: None,
            nested_fields: vec![],
        }],
        options: vec![],
        schema_ref: None,
        example: None,
    }),
    // ...
};
let v = parse_json(&render_endpoint_detail(&endpoint_with_new_fields, false));
let fields = &v["request_body"]["fields"][0];
assert_eq!(fields["write_only"], true);
assert_eq!(fields["deprecated"], false);
assert!(fields["constraints"].is_array());
```

---

### Task 21 — JSON renderer: update endpoint detail for headers, links, callbacks

**File:** `src/render/json.rs`
**Depends on:** Tasks 9, 10, 11

The endpoint JSON renderer currently uses `serialize(endpoint, is_tty)` which directly serializes the `Endpoint` struct using its `#[derive(Serialize)]`. This means the new `headers`, `links`, and `callbacks` fields on the model (Tasks 2, 3, 4) will automatically appear in JSON output when their respective model tasks are complete.

**No code change needed here** — the JSON renderer for endpoints passes through the struct directly, and the new fields are already marked `#[derive(serde::Serialize)]`.

**Verification test:**

```rust
#[test]
fn test_endpoint_json_includes_new_fields() {
    use crate::models::resource::*;

    let endpoint = Endpoint {
        method: "GET".to_string(),
        path: "/users".to_string(),
        summary: None,
        description: None,
        is_deprecated: false,
        is_alpha: false,
        external_docs: None,
        parameters: vec![],
        request_body: None,
        responses: vec![Response {
            status_code: "200".to_string(),
            description: "OK".to_string(),
            schema_ref: None,
            example: None,
            headers: vec![ResponseHeader {
                name: "X-Total-Count".to_string(),
                type_display: "integer".to_string(),
                description: None,
            }],
            links: vec![],
        }],
        security_schemes: vec![],
        callbacks: vec![],
        links: vec![],
        drill_deeper: vec![],
    };

    let v = parse_json(&render_endpoint_detail(&endpoint, false));
    assert!(v["responses"][0]["headers"].is_array(), "headers should be array in JSON");
    assert_eq!(v["responses"][0]["headers"][0]["name"], "X-Total-Count");
    assert!(v["callbacks"].is_array(), "callbacks should be present as array");
    assert!(v["links"].is_array(), "links should be present as array");
}
```

---

### Task 22 — New `callbacks` subcommand: renderers + CLI wiring

**Files:** `src/render/text.rs`, `src/render/json.rs`, `src/main.rs`
**Depends on:** Task 12

#### Part A: Text renderer

Add to `src/render/text.rs`:

```rust
pub fn render_callback_list(callbacks: &[crate::models::resource::CallbackEntry], is_tty: bool) -> String {
    let mut out = String::new();
    if callbacks.is_empty() {
        out.push_str("Callbacks: (none)\n");
        return out;
    }
    writeln!(out, "Callbacks ({} total):", callbacks.len()).unwrap();
    for cb in callbacks {
        writeln!(
            out,
            "  {}  (on {} {})",
            sanitize(&cb.name),
            sanitize(&cb.defined_on_method),
            sanitize(&cb.defined_on_path)
        ).unwrap();
    }
    if is_tty {
        out.push_str("\nDrill deeper:\n");
        out.push_str("  phyllotaxis callbacks <name>\n");
    }
    out
}

pub fn render_callback_detail(cb: &crate::models::resource::CallbackEntry, is_tty: bool) -> String {
    let mut out = String::new();
    writeln!(out, "Callback: {}", sanitize(&cb.name)).unwrap();
    writeln!(
        out,
        "Defined on: {} {}",
        sanitize(&cb.defined_on_method),
        sanitize(&cb.defined_on_path)
    ).unwrap();

    for op in &cb.operations {
        writeln!(out, "\n  {} {}", sanitize(&op.method), sanitize(&op.url_expression)).unwrap();
        if let Some(ref schema) = op.body_schema {
            writeln!(out, "    Body: {}", sanitize(schema)).unwrap();
        }
        if !op.responses.is_empty() {
            out.push_str("    Responses:\n");
            for r in &op.responses {
                writeln!(out, "      {}  {}", sanitize(&r.status_code), sanitize(&r.description)).unwrap();
            }
        }
    }

    if is_tty {
        let schema_names: Vec<&str> = cb.operations.iter()
            .filter_map(|op| op.body_schema.as_deref())
            .filter(|s| *s != "inline object")
            .collect();
        if !schema_names.is_empty() {
            out.push_str("\nDrill deeper:\n");
            for name in schema_names {
                writeln!(out, "  phyllotaxis schemas {}", sanitize(name)).unwrap();
            }
        }
    }

    out
}
```

#### Part B: JSON renderer

Add to `src/render/json.rs`:

```rust
pub fn render_callback_list(callbacks: &[crate::models::resource::CallbackEntry], is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct CallbackListJson<'a> {
        total: usize,
        callbacks: Vec<CallbackSummaryJson<'a>>,
        drill_deeper: &'static str,
    }
    #[derive(serde::Serialize)]
    struct CallbackSummaryJson<'a> {
        name: &'a str,
        defined_on_method: &'a str,
        defined_on_path: &'a str,
    }

    let items: Vec<_> = callbacks.iter().map(|cb| CallbackSummaryJson {
        name: &cb.name,
        defined_on_method: &cb.defined_on_method,
        defined_on_path: &cb.defined_on_path,
    }).collect();

    let json = CallbackListJson {
        total: items.len(),
        callbacks: items,
        drill_deeper: "phyllotaxis callbacks <name>",
    };
    serialize(&json, is_tty)
}

pub fn render_callback_detail(cb: &crate::models::resource::CallbackEntry, is_tty: bool) -> String {
    serialize(cb, is_tty)
}
```

#### Part C: CLI wiring

In `src/main.rs`, add the new subcommand to the `Commands` enum:

```rust
/// List all callbacks, or show detail for a specific callback
Callbacks {
    /// Callback name to drill into
    name: Option<String>,
},
```

In the `match &cli.command` block:

```rust
Some(Commands::Callbacks { name }) => {
    let callbacks = commands::callbacks::list_all_callbacks(&loaded.api);
    match name {
        None => {
            let output = if cli.json {
                render::json::render_callback_list(&callbacks, is_tty)
            } else {
                render::text::render_callback_list(&callbacks, is_tty)
            };
            println!("{}", output);
        }
        Some(name) => {
            match commands::callbacks::find_callback(&loaded.api, name) {
                Some(cb) => {
                    let output = if cli.json {
                        render::json::render_callback_detail(&cb, is_tty)
                    } else {
                        render::text::render_callback_detail(&cb, is_tty)
                    };
                    println!("{}", output);
                }
                None => {
                    if cli.json {
                        eprintln!("{}", json_error(&format!("Callback '{}' not found.", name)));
                    } else {
                        eprintln!("Error: Callback '{}' not found.", name);
                    }
                    std::process::exit(1);
                }
            }
        }
    }
}
```

The `Init` and `Completions` variants use early `if let` guards that return before reaching the main match block — they are NOT match arms. Do NOT add an `unreachable!()` arm. The new `Some(Commands::Callbacks { name }) => { ... }` arm shown above is a normal arm in the main match; add it before the existing `unreachable!()` arms for `Init` and `Completions` at lines 252–253.

**Also update the `render_overview` text output** to advertise the new subcommand:

```rust
// In render_overview in text.rs, add after the existing commands:
out.push_str("  phyllotaxis callbacks    List all webhook callbacks\n");
```

**Test to write first:**

```rust
#[test]
fn test_render_callback_list() {
    use crate::models::resource::*;
    let callbacks = vec![
        CallbackEntry {
            name: "onEvent".to_string(),
            defined_on_operation_id: Some("subscribeNotifications".to_string()),
            defined_on_method: "POST".to_string(),
            defined_on_path: "/notifications/subscribe".to_string(),
            operations: vec![],
        },
    ];
    let output = render_callback_list(&callbacks, true);
    assert!(output.contains("Callbacks"), "Missing header");
    assert!(output.contains("onEvent"), "Missing callback name");
    assert!(output.contains("phyllotaxis callbacks <name>"), "Missing drill hint");
}

#[test]
fn test_render_callback_detail() {
    use crate::models::resource::*;
    let cb = CallbackEntry {
        name: "onEvent".to_string(),
        defined_on_operation_id: Some("subscribeNotifications".to_string()),
        defined_on_method: "POST".to_string(),
        defined_on_path: "/notifications/subscribe".to_string(),
        operations: vec![CallbackOperation {
            method: "POST".to_string(),
            url_expression: "{$request.query.callbackUrl}/events".to_string(),
            summary: None,
            body_schema: Some("EventPayload".to_string()),
            responses: vec![
                CallbackResponse { status_code: "200".to_string(), description: "OK".to_string() },
            ],
        }],
    };
    let output = render_callback_detail(&cb, false);
    assert!(output.contains("Callback: onEvent"), "Missing callback name, got:\n{}", output);
    assert!(output.contains("POST /notifications/subscribe"), "Missing defined-on line, got:\n{}", output);
    assert!(output.contains("EventPayload"), "Missing body schema, got:\n{}", output);
    assert!(output.contains("200"), "Missing response code, got:\n{}", output);
}
```

**Integration tests** (in `tests/integration_tests.rs`):

```rust
fn run_with_kitchen_sink(args: &[&str]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/kitchen-sink.yaml", manifest_dir);
    let mut full_args = vec!["--spec", &spec];
    full_args.extend_from_slice(args);
    run(&full_args)
}

#[test]
fn test_callbacks_list_kitchen_sink() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["callbacks"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(stdout.contains("onEvent"), "Missing onEvent callback");
    assert!(stdout.contains("onStatusChange"), "Missing onStatusChange callback");
}

#[test]
fn test_callbacks_detail_on_event() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["callbacks", "onEvent"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(stdout.contains("Callback: onEvent"), "Missing header");
    assert!(stdout.contains("EventPayload"), "Missing body schema");
}

#[test]
fn test_callbacks_not_found() {
    let (_stdout, stderr, code) = run_with_kitchen_sink(&["callbacks", "nonexistent"]);
    assert_eq!(code, 1, "Expected exit code 1");
    assert!(stderr.contains("not found"), "Missing not found message");
}

#[test]
fn test_multipart_body_visible_in_upload_endpoint() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "files", "POST", "/files/upload"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(stdout.contains("multipart/form-data"), "Missing content type, got:\n{}", &stdout[..300.min(stdout.len())]);
    assert!(stdout.contains("file"), "Missing file field, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_response_headers_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "GET", "/users"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("X-Total-Count"), "Missing response header, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_links_visible_on_post_users() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["resources", "users", "POST", "/users"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("GetCreatedUser"), "Missing link, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_schema_constraints_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "User"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("min:3"), "Missing min constraint, got:\n{}", &stdout[..300.min(stdout.len())]);
    assert!(stdout.contains("max:32"), "Missing max constraint, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_schema_title_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "GeoLocation"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Geographic Location"), "Missing title, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_integer_enum_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "Priority"]);
    assert_eq!(code, 0);
    // Should show [0, 1, 2, 3, 4]
    assert!(stdout.contains('0'), "Missing integer enum value, got:\n{}", &stdout[..300.min(stdout.len())]);
    assert!(stdout.contains('4'), "Missing integer enum value 4, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_write_only_visible_on_create_user_request() {
    // Design success criterion #7: CreateUserRequest — password shows [write-only]
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "CreateUserRequest"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("write-only"), "Missing write-only on password field, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_deprecated_visible_on_pet_base() {
    // Design success criterion #8: PetBase — legacy_code shows [DEPRECATED]
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "PetBase"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("DEPRECATED"), "Missing DEPRECATED on legacy_code, got:\n{}", &stdout[..300.min(stdout.len())]);
}

#[test]
fn test_petstore_regression() {
    // All existing petstore tests should still pass — smoke test
    let (stdout, _stderr, code) = run_with_petstore(&["resources", "pets", "POST", "/pets"]);
    assert_eq!(code, 0, "Petstore regression: POST /pets should still work");
    assert!(stdout.contains("Request Body"), "Regression: missing request body");
}
```

---

## Verification Commands

After implementing all tasks, run:

```bash
# All unit tests (including all pre-existing tests — success criterion #11: no regressions)
cargo test --lib

# All integration tests
cargo test --test integration_tests

# Specific coverage check — all 11 success criteria from design doc
cargo test --test integration_tests test_multipart_body_visible_in_upload_endpoint    # criterion 1
cargo test --test integration_tests test_response_headers_visible                     # criterion 2
cargo test --test integration_tests test_callbacks_list_kitchen_sink                  # criterion 3
cargo test --test integration_tests test_callbacks_detail_on_event                    # criterion 4
cargo test --test integration_tests test_links_visible_on_post_users                  # criterion 5
cargo test --test integration_tests test_schema_constraints_visible                   # criterion 6
cargo test --test integration_tests test_write_only_visible_on_create_user_request    # criterion 7
cargo test --test integration_tests test_deprecated_visible_on_pet_base               # criterion 8
cargo test --test integration_tests test_schema_title_visible                         # criterion 9
cargo test --test integration_tests test_integer_enum_visible                         # criterion 10
cargo test --test integration_tests test_petstore_regression                          # criterion 11

# Manual smoke test against kitchen-sink
cargo run -- --spec tests/fixtures/kitchen-sink.yaml resources files POST /files/upload
cargo run -- --spec tests/fixtures/kitchen-sink.yaml resources users GET /users
cargo run -- --spec tests/fixtures/kitchen-sink.yaml resources notifications POST /notifications/subscribe
cargo run -- --spec tests/fixtures/kitchen-sink.yaml callbacks
cargo run -- --spec tests/fixtures/kitchen-sink.yaml callbacks onEvent
cargo run -- --spec tests/fixtures/kitchen-sink.yaml resources users POST /users
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas User
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas CreateUserRequest
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas PetBase
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas GeoLocation
cargo run -- --spec tests/fixtures/kitchen-sink.yaml schemas Priority

# Regression: petstore still works
cargo run -- --spec tests/fixtures/petstore.yaml resources pets POST /pets
```

---

## Implementation Order Summary

1. Task 1 — extend `Field` struct
2. Task 2 — `ResponseHeader` + extend `Response`
3. Task 3 — `ResponseLink` + `links` on `Response`
4. Task 4 — `CallbackEntry` + `callbacks`/`links` on `Endpoint`
5. Task 5 — `title` on `SchemaModel`
6. `cargo check` — fix all construction sites broken by struct changes
7. Task 6 — extract `write_only` + `deprecated`
8. Task 7 — extract constraints
9. Task 8 — integer enum extraction
10. Task 9 — response header extraction
11. Task 10 — link extraction
12. Task 11 — callback inline extraction
13. Task 12 — new `callbacks.rs` module
14. Task 13 — schema title extraction
15. Task 14 — multi-content-type request body
16. `cargo test` — all extraction tests should now pass
17. Task 15 — text rendering: field annotations
18. Task 16 — text rendering: response headers
19. Task 17 — text rendering: links
20. Task 18 — text rendering: callbacks inline
21. Task 19 — text rendering: schema title
22. Task 20 — JSON: field struct updates
23. Task 21 — JSON: verify passthrough (no change needed)
24. Task 22 — callbacks subcommand: renderers + CLI wiring
25. `cargo test` — full suite
26. Integration tests against kitchen-sink
