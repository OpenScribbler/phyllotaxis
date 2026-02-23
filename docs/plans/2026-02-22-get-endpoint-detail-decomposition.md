# Plan: Decompose `get_endpoint_detail`
**Date:** 2026-02-22
**File:** `src/commands/resources.rs`
**Function:** `pub fn get_endpoint_detail` (lines 285–497, ~213 lines)

---

## Current structure

`get_endpoint_detail` is a single 213-line function with six distinct responsibilities executed sequentially:

1. **Resolve path item** — look up the path string in `api.paths.paths`, dereference `ReferenceOr`
2. **Resolve operation** — select the operation from the path item by HTTP method string
3. **Merge and extract parameters** — merge path-level and operation-level params (operation wins on conflict), extract schema type info for each via `extract_param_schema_info`
4. **Extract request body** — find `application/json` media, handle `oneOf`/`anyOf` variant names vs. concrete fields, optionally expand fields recursively
5. **Extract responses** — iterate `operation.responses`, resolve status codes, pull schema ref names and examples
6. **Extract security schemes** — merge operation-level security with API-level security, collect scheme names

Then it assembles all extracted data into an `Endpoint` struct.

---

## Proposed decomposition

Extract each responsibility into a private helper function. `get_endpoint_detail` becomes an orchestrator that calls helpers and assembles the result.

### New private functions

#### `fn resolve_path_item<'a>`
```rust
fn resolve_path_item<'a>(
    api: &'a openapiv3::OpenAPI,
    path: &str,
) -> Option<&'a openapiv3::PathItem>
```
Looks up `path` in `api.paths.paths`, unwraps `ReferenceOr::Item`. Returns `None` for references or missing paths.

**Extracted from:** lines 293–297

---

#### `fn resolve_operation<'a>`
```rust
fn resolve_operation<'a>(
    path_item: &'a openapiv3::PathItem,
    method: &str,
) -> Option<&'a openapiv3::Operation>
```
Maps the method string (`"GET"`, `"POST"`, etc.) to the appropriate field on `PathItem`. Returns `None` for unknown methods or absent operations.

**Extracted from:** lines 299–310

---

#### `fn merge_parameters`
```rust
fn merge_parameters(
    api: &openapiv3::OpenAPI,
    path_item: &openapiv3::PathItem,
    operation: &openapiv3::Operation,
) -> Vec<Parameter>
```
Merges `path_item.parameters` and `operation.parameters` (operation wins on name collision). Calls the existing `extract_param_schema_info`. Returns a `Vec<Parameter>` sorted by insertion order (current behavior uses `BTreeMap` keyed by name — preserve this).

**Extracted from:** lines 312–359
**Imports needed inside:** `use crate::models::resource::{Parameter, ParameterLocation};`

---

#### `fn extract_request_body`
```rust
fn extract_request_body(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
    expand: bool,
) -> Option<RequestBody>
```
Finds `application/json` in `operation.request_body`, handles `oneOf`/`anyOf` variant names vs. concrete field extraction, optionally calls `expand_fields_pub`. This is the most complex helper — it stays complex internally, but isolating it makes that complexity contained and independently testable.

**Extracted from:** lines 361–432
**Imports needed inside:** `use crate::models::resource::{RequestBody};`

---

#### `fn extract_responses`
```rust
fn extract_responses(
    operation: &openapiv3::Operation,
) -> Vec<Response>
```
Iterates `operation.responses.responses`, formats status codes, resolves schema ref names and examples from `application/json` media. Skips reference responses (as current code does).

**Extracted from:** lines 434–467
**Imports needed inside:** `use crate::models::resource::Response;`

---

#### `fn extract_security`
```rust
fn extract_security(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
) -> Vec<String>
```
Returns security scheme names from operation-level security, falling back to API-level security. Short function — ~10 lines.

**Extracted from:** lines 469–479

---

### Resulting `get_endpoint_detail`

After decomposition:

```rust
pub fn get_endpoint_detail(
    api: &openapiv3::OpenAPI,
    method: &str,
    path: &str,
    expand: bool,
) -> Option<Endpoint> {
    let path_item = resolve_path_item(api, path)?;
    let operation = resolve_operation(path_item, method)?;

    let parameters = merge_parameters(api, path_item, operation);
    let request_body = extract_request_body(api, operation, expand);
    let responses = extract_responses(operation);
    let security_schemes = extract_security(api, operation);

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
}
```

~20 lines. Each responsibility is named and testable independently.

---

## TDD approach

This is a refactor — external behavior is unchanged. Follow the characterize → refactor → verify pattern:

### Step 1 — Characterize current behavior (before any changes)

Add unit tests directly calling the new helper functions — but since they don't exist yet, these tests will guide the extraction:

1. **`resolve_path_item`:** Test with a known path (returns `Some`), unknown path (returns `None`), and a path item that is a `Reference` (returns `None`).
2. **`resolve_operation`:** Test each HTTP method string, an unknown method (`"CONNECT"`), and a method that exists in the path item vs. one that doesn't.
3. **`merge_parameters`:** Test that operation-level params override path-level params with the same name. Test that `required` and `location` are preserved correctly.
4. **`extract_request_body`:** Test `oneOf` variant extraction, concrete field extraction, and `expand=true` behavior.
5. **`extract_responses`:** Test multiple status codes, `2XX` range codes, responses with and without schema refs.
6. **`extract_security`:** Test operation-level security overrides API-level, fallback to API-level when operation has none.

### Step 2 — Extract helpers

Extract one function at a time, in the order listed above (simpler ones first). After each extraction, run `cargo test` — all 119 tests must pass.

### Step 3 — Final verification

Run `cargo test` on the completed refactor. Output of the integration tests must be byte-for-byte identical to before.

---

## Risk assessment

| Risk | Mitigation |
|------|-----------|
| Lifetime complexity in `resolve_path_item` (returns `&'a PathItem`) | Start with this one — it's the simplest and confirms the lifetime approach works |
| `extract_request_body` shares logic with `build_fields` / `expand_fields_pub` | Those functions are unchanged; just call them the same way |
| Parameter merge order (BTreeMap sorts by key name, not insertion order) | Current behavior already sorts by name; preserve by using `BTreeMap::into_values()` |
| Integration test output changes | Run `cargo test` after each extraction; any change is a bug |

---

## Suggested execution order

Extract in this order to minimize risk — simple functions first, most complex last:

1. `resolve_path_item` — 5 lines, no complexity
2. `resolve_operation` — 10 lines, straightforward match
3. `extract_security` — ~10 lines, simple
4. `extract_responses` — ~30 lines, moderate
5. `merge_parameters` — ~50 lines, calls existing helper
6. `extract_request_body` — ~70 lines, most complex; save for last

After each: `cargo test` must be green before moving to the next.
