# Agent Execution Plan: `get_endpoint_detail` Decomposition
**Date:** 2026-02-22
**File:** `src/commands/resources.rs`
**Design doc:** `docs/plans/2026-02-22-get-endpoint-detail-decomposition.md`
**Baseline:** 119 tests passing before Bead 1 begins

---

## Two-agent model per bead

Each bead runs two agents sequentially:

**Implementing agent** — makes the code changes:
1. Read `src/commands/resources.rs` in full before making any change.
2. Write the test first. Confirm it fails to compile or fails to run (RED state). Do NOT run `cargo test` to verify your own work after implementing — that is the verifier's job.
3. Extract the function. Replace the extracted lines in `get_endpoint_detail` with a call to the new function.
4. Do not refactor, rename, or change any logic. Move code exactly as-is.
5. Report: exact lines changed, new function added, new test added. State clearly: "Implementation complete — awaiting verification."

**Verifier agent** — independently checks the result:
1. Read `src/commands/resources.rs` to confirm the implementing agent's changes look correct (new function present, correct call site, new test present).
2. Run `cargo test` from the project root.
3. Report: pass/fail count, any failures with full output.
4. If tests fail: describe the failure precisely. Do NOT fix it — that goes back to the implementing agent.

The implementing agent does NOT verify. The verifier agent does NOT implement. They are separate runs.

---

## Bead 1 — Extract `resolve_path_item`

**What it does:** Looks up a path string in `api.paths.paths`, unwraps `ReferenceOr::Item`. Returns `None` for references or missing paths.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 293–297:
```rust
// 1. Find path item
let path_item = match api.paths.paths.get(path)? {
    openapiv3::ReferenceOr::Item(item) => item,
    openapiv3::ReferenceOr::Reference { .. } => return None,
};
```

### Step 1 — Write the test (RED)

Add this test inside the `#[cfg(test)]` module at the bottom of `src/commands/resources.rs`:

```rust
#[test]
fn test_resolve_path_item_found() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let result = resolve_path_item(&api.api, "/pets");
    assert!(result.is_some(), "expected /pets to resolve to a path item");
}

#[test]
fn test_resolve_path_item_missing() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let result = resolve_path_item(&api.api, "/does-not-exist");
    assert!(result.is_none());
}
```

Run `cargo test` — the tests should fail to compile because `resolve_path_item` doesn't exist yet. That's the RED state.

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add this private function to `src/commands/resources.rs` (place it just before `get_endpoint_detail`):

```rust
fn resolve_path_item<'a>(
    api: &'a openapiv3::OpenAPI,
    path: &str,
) -> Option<&'a openapiv3::PathItem> {
    match api.paths.paths.get(path)? {
        openapiv3::ReferenceOr::Item(item) => Some(item),
        openapiv3::ReferenceOr::Reference { .. } => None,
    }
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 1. Find path item
let path_item = resolve_path_item(api, path)?;
```

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `resolve_path_item` function is present just before `get_endpoint_detail`
- The extracted lines in `get_endpoint_detail` are replaced with `resolve_path_item(api, path)?`
- The two new tests are present in the `#[cfg(test)]` block

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report the exact failure output — do not fix.

---

## Bead 2 — Extract `resolve_operation`

**Prerequisite:** Bead 1 complete and passing.

**What it does:** Maps an HTTP method string to the corresponding operation on a `PathItem`. Returns `None` for unknown methods or absent operations.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 299–310:
```rust
// 2. Get operation by method
let operation = match method.to_uppercase().as_str() {
    "GET" => path_item.get.as_ref(),
    "POST" => path_item.post.as_ref(),
    "PUT" => path_item.put.as_ref(),
    "DELETE" => path_item.delete.as_ref(),
    "PATCH" => path_item.patch.as_ref(),
    "HEAD" => path_item.head.as_ref(),
    "OPTIONS" => path_item.options.as_ref(),
    "TRACE" => path_item.trace.as_ref(),
    _ => None,
}?;
```

### Step 1 — Write the test (RED)

```rust
#[test]
fn test_resolve_operation_known_method() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    // petstore has GET /pets
    assert!(resolve_operation(path_item, "GET").is_some());
    assert!(resolve_operation(path_item, "get").is_some(), "case-insensitive");
}

#[test]
fn test_resolve_operation_unknown_method() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    assert!(resolve_operation(path_item, "CONNECT").is_none());
}

#[test]
fn test_resolve_operation_absent_method() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    // petstore likely has no DELETE /pets — confirm it returns None
    // (check the fixture; if DELETE /pets exists, use a different method)
    assert!(resolve_operation(path_item, "DELETE").is_none());
}
```

**Note:** Before writing these tests, read the petstore fixture to confirm which methods exist on `/pets`. Adjust the absent-method test if needed.

Run `cargo test` — RED (function doesn't exist yet).

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add before `get_endpoint_detail`:

```rust
fn resolve_operation<'a>(
    path_item: &'a openapiv3::PathItem,
    method: &str,
) -> Option<&'a openapiv3::Operation> {
    match method.to_uppercase().as_str() {
        "GET"     => path_item.get.as_ref(),
        "POST"    => path_item.post.as_ref(),
        "PUT"     => path_item.put.as_ref(),
        "DELETE"  => path_item.delete.as_ref(),
        "PATCH"   => path_item.patch.as_ref(),
        "HEAD"    => path_item.head.as_ref(),
        "OPTIONS" => path_item.options.as_ref(),
        "TRACE"   => path_item.trace.as_ref(),
        _         => None,
    }
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 2. Get operation by method
let operation = resolve_operation(path_item, method)?;
```

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `resolve_operation` function is present
- The extracted lines in `get_endpoint_detail` are replaced with `resolve_operation(path_item, method)?`
- The new tests are present

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report exact output — do not fix.

---

## Bead 3 — Extract `extract_security`

**Prerequisite:** Bead 2 complete and passing.

**What it does:** Collects security scheme names from operation-level security, falling back to API-level security when the operation has none.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 469–479:
```rust
// 6. Security schemes
let security = operation
    .security
    .as_ref()
    .or(api.security.as_ref())
    .map(|reqs| {
        reqs.iter()
            .flat_map(|req| req.keys().cloned())
            .collect::<Vec<String>>()
    })
    .unwrap_or_default();
```

### Step 1 — Write the test (RED)

```rust
#[test]
fn test_extract_security_from_operation() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    // Just verify the function runs without panic and returns a Vec
    let security = extract_security(&api.api, operation);
    // petstore may or may not have security on this endpoint — either is valid
    let _ = security; // not asserting content, just that it doesn't panic
}
```

Run `cargo test` — RED.

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add before `get_endpoint_detail`:

```rust
fn extract_security(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
) -> Vec<String> {
    operation
        .security
        .as_ref()
        .or(api.security.as_ref())
        .map(|reqs| {
            reqs.iter()
                .flat_map(|req| req.keys().cloned())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 6. Security schemes
let security_schemes = extract_security(api, operation);
```

Also update the `Endpoint` struct construction: change `security_schemes: security` to `security_schemes` (shorthand, variable name matches field name).

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `extract_security` function is present
- The extracted block in `get_endpoint_detail` is replaced with `let security_schemes = extract_security(api, operation);`
- `Endpoint` construction uses `security_schemes` shorthand
- The new test is present

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report exact output — do not fix.

---

## Bead 4 — Extract `extract_responses`

**Prerequisite:** Bead 3 complete and passing.

**What it does:** Iterates `operation.responses.responses`, formats status codes (`200` → `"200"`, `2` → `"2XX"`), extracts schema ref names and examples from `application/json` media. Skips reference responses.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 434–467:
```rust
// 5. Responses
let mut responses = Vec::new();
for (status, resp_ref) in &operation.responses.responses {
    // ... full block ...
}
```

### Step 1 — Write the test (RED)

```rust
#[test]
fn test_extract_responses_nonempty() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    let responses = extract_responses(operation);
    assert!(!responses.is_empty(), "GET /pets should have at least one response");
}

#[test]
fn test_extract_responses_status_codes_are_strings() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    let responses = extract_responses(operation);
    for r in &responses {
        assert!(!r.status_code.is_empty());
    }
}
```

Run `cargo test` — RED.

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add before `get_endpoint_detail`:

```rust
fn extract_responses(operation: &openapiv3::Operation) -> Vec<crate::models::resource::Response> {
    use crate::models::resource::Response;
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

        responses.push(Response {
            status_code,
            description: resp.description.clone(),
            schema_ref: schema_ref_name,
            example,
        });
    }
    responses
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 5. Responses
let responses = extract_responses(operation);
```

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `extract_responses` function is present
- The extracted block in `get_endpoint_detail` is replaced with `let responses = extract_responses(operation);`
- The new tests are present

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report exact output — do not fix.

---

## Bead 5 — Extract `merge_parameters`

**Prerequisite:** Bead 4 complete and passing.

**What it does:** Merges path-level and operation-level parameters (operation wins on name conflict). Calls the existing `extract_param_schema_info` helper for schema type info. Returns a `Vec<Parameter>`.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 312–359:
```rust
// 3. Merge parameters: path-level then operation-level (operation wins)
let mut params_map: std::collections::BTreeMap<String, Parameter> = ...
// ... full block through the closing brace of the for loop ...
```

### Step 1 — Write the test (RED)

```rust
#[test]
fn test_merge_parameters_nonempty_for_parameterized_path() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    // /pets/{id} or similar should have path parameters — check the fixture
    // Use whatever parameterized path exists in petstore
    let path_item = resolve_path_item(&api.api, "/pets/{id}").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    let params = merge_parameters(&api.api, path_item, operation);
    assert!(!params.is_empty(), "parameterized endpoint should have parameters");
}

#[test]
fn test_merge_parameters_empty_for_simple_path() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    // GET /pets may have query params (limit, etc.) or none — just verify it runs
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    let params = merge_parameters(&api.api, path_item, operation);
    let _ = params; // just verify it doesn't panic
}
```

**Note:** Before writing these tests, read the petstore fixture to find the correct parameterized path. Adjust the path string (`"/pets/{id}"`) to match what exists.

Run `cargo test` — RED.

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add before `get_endpoint_detail`:

```rust
fn merge_parameters(
    api: &openapiv3::OpenAPI,
    path_item: &openapiv3::PathItem,
    operation: &openapiv3::Operation,
) -> Vec<crate::models::resource::Parameter> {
    use crate::models::resource::{Parameter, ParameterLocation};
    let mut params_map: std::collections::BTreeMap<String, Parameter> =
        std::collections::BTreeMap::new();

    let all_param_refs: Vec<&openapiv3::ReferenceOr<openapiv3::Parameter>> = path_item
        .parameters
        .iter()
        .chain(operation.parameters.iter())
        .collect();

    for param_ref in all_param_refs {
        let param = match param_ref {
            openapiv3::ReferenceOr::Item(p) => p,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        let data = match param {
            openapiv3::Parameter::Query { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Query)
            }
            openapiv3::Parameter::Path { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Path)
            }
            openapiv3::Parameter::Header { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Header)
            }
            _ => continue,
        };

        let (pdata, location) = data;
        let (schema_type, format, enum_values) =
            extract_param_schema_info(api, &pdata.format);

        params_map.insert(
            pdata.name.clone(),
            Parameter {
                name: pdata.name.clone(),
                location,
                required: pdata.required,
                schema_type,
                format,
                description: pdata.description.clone(),
                enum_values,
            },
        );
    }

    params_map.into_values().collect()
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 3. Merge parameters
let parameters = merge_parameters(api, path_item, operation);
```

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `merge_parameters` function is present
- The extracted block in `get_endpoint_detail` is replaced with `let parameters = merge_parameters(api, path_item, operation);`
- The new tests are present

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report exact output — do not fix.

---

## Bead 6 — Extract `extract_request_body`

**Prerequisite:** Bead 5 complete and passing.

**What it does:** Finds `application/json` content in the request body, handles `oneOf`/`anyOf` variant surface vs. concrete field extraction, optionally expands fields. The most complex of the helpers.

**Lines to extract from `get_endpoint_detail`:** The block currently at approximately lines 361–432:
```rust
// 4. Request body
let request_body = operation.request_body.as_ref().and_then(|rb_ref| {
    // ... full closure body ...
});
```

### Step 1 — Write the test (RED)

```rust
#[test]
fn test_extract_request_body_post_creates_pet() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    // Find a POST endpoint that has a request body — check the fixture
    // Use whatever POST path exists in petstore
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "POST").unwrap();
    let body = extract_request_body(&api.api, operation, false);
    // POST /pets should have a request body if petstore includes one
    // Adjust assertion based on what the fixture actually has
    let _ = body;
}

#[test]
fn test_extract_request_body_get_returns_none() {
    let api = crate::spec::load_spec(
        Some("tests/fixtures/petstore.yaml"),
        &std::path::PathBuf::from("."),
    )
    .unwrap();
    let path_item = resolve_path_item(&api.api, "/pets").unwrap();
    let operation = resolve_operation(path_item, "GET").unwrap();
    // GET requests should have no request body
    let body = extract_request_body(&api.api, operation, false);
    assert!(body.is_none(), "GET should not have a request body");
}
```

**Note:** Check the petstore fixture first to confirm which endpoints have request bodies. Adjust accordingly.

Run `cargo test` — RED.

### Step 2 — Extract the function (GREEN) [Implementing agent]

Add before `get_endpoint_detail`:

```rust
fn extract_request_body(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
    expand: bool,
) -> Option<crate::models::resource::RequestBody> {
    use crate::models::resource::RequestBody;

    operation.request_body.as_ref().and_then(|rb_ref| {
        let rb = match rb_ref {
            openapiv3::ReferenceOr::Item(rb) => rb,
            openapiv3::ReferenceOr::Reference { .. } => return None,
        };

        let media = rb.content.get("application/json")?;
        let schema_ref = media.schema.as_ref()?;

        let schema: &openapiv3::Schema = match schema_ref {
            openapiv3::ReferenceOr::Item(s) => s,
            openapiv3::ReferenceOr::Reference { reference } => {
                let sname = spec::schema_name_from_ref(reference)?;
                let components = api.components.as_ref()?;
                match components.schemas.get(sname)? {
                    openapiv3::ReferenceOr::Item(s) => s,
                    _ => return None,
                }
            }
        };

        let example = media.example.clone();

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
                content_type: "application/json".to_string(),
                fields: Vec::new(),
                options,
                example,
            });
        }

        let required: Vec<String> =
            if let openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) = &schema.schema_kind
            {
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
            content_type: "application/json".to_string(),
            fields,
            options: Vec::new(),
            example,
        })
    })
}
```

In `get_endpoint_detail`, replace the extracted block with:
```rust
// 4. Request body
let request_body = extract_request_body(api, operation, expand);
```

Also update the `Endpoint` struct construction to use shorthand where variable names now match field names:
```rust
Some(Endpoint {
    method: method.to_uppercase(),
    path: path.to_string(),
    summary: operation.summary.clone(),
    description: operation.description.clone(),
    is_deprecated: operation.deprecated,
    is_alpha: matches!(
        operation.extensions.get("x-alpha"),
        Some(serde_json::Value::Bool(true))
    ),
    external_docs: None,
    parameters,
    request_body,
    responses,
    security_schemes,
})
```

Report what you changed. State: "Implementation complete — awaiting verification." Do not run `cargo test`.

### Step 3 — Verify [Verifier agent]

Read `src/commands/resources.rs`. Confirm:
- `extract_request_body` function is present
- The extracted block in `get_endpoint_detail` is replaced with `let request_body = extract_request_body(api, operation, expand);`
- `get_endpoint_detail` is now approximately 20 lines of orchestration
- The new tests are present
- All 6 private helper functions are present above `get_endpoint_detail`: `resolve_path_item`, `resolve_operation`, `extract_security`, `extract_responses`, `merge_parameters`, `extract_request_body`

Run `cargo test` from `/home/hhewett/.local/src/phyllotaxis`. Report full pass/fail count. If any tests fail, report exact output — do not fix.

---

## Completion criteria

After Bead 6:
- `get_endpoint_detail` is ~20 lines of orchestration
- 6 private helper functions are defined above it in the file
- All tests pass (119 original + new tests added across beads)
- `cargo clippy -- -D warnings` passes clean
- No logic has changed — only code organization
