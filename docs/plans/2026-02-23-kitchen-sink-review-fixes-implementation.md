# Kitchen-Sink Review Fixes - Implementation Plan

**Goal:** Fix all 10 issues from post-implementation review

**Architecture:** Leaf-node changes to extraction (commands/) and rendering (render/) layers. No model restructuring.

**Tech Stack:** Rust, openapiv3 crate, serde_json

**Design Doc:** docs/plans/2026-02-23-kitchen-sink-review-fixes-design.md

---

## Overview

Ten independent issues. Most share no dependencies and can be worked in any order. The one exception: Task 10 (callback count in overview) depends on the `OverviewData` struct change, so both text and JSON rendering for it must be done together.

**Recommended order:** 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10

---

## Task 1: Empty Request Body Display for Non-Schema Content Types

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (request body section in `render_endpoint_detail`)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Note on JSON:** The JSON renderer calls `serialize(endpoint, is_tty)` directly on the `Endpoint` struct via serde. The `request_body` field already includes `content_type`, `fields` (as `[]`), and `options` (as `[]`). When `fields` is empty, JSON output shows `"fields": []` — which is correct and informative for machine consumers. A consumer can distinguish "no schema was present" from "has fields" by checking the empty array. The "Raw body (no schema)" message is a text-only UX improvement and no JSON change is needed.

**Depends on:** None

**Success Criteria:**
- [ ] `POST /admin/bulk-import` text output shows `Raw body (no schema)` instead of blank space after the `Request Body (text/csv):` header
- [ ] Petstore endpoints with JSON bodies are unaffected

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs, add after the last kitchen-sink test:

#[test]
fn test_raw_body_shown_for_csv_content_type() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["resources", "admin", "POST", "/admin/bulk-import"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(
        stdout.contains("text/csv"),
        "Missing content type, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("Raw body (no schema)"),
        "Expected 'Raw body (no schema)' for CSV body with no parseable fields, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_raw_body_shown_for_csv_content_type -- --nocapture`
Expected: FAIL (output shows `Request Body (text/csv):` followed by nothing)

### Step 3: Write minimal implementation

In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`, find the request body section in `render_endpoint_detail`. The current code (starting around line 132):

```rust
    // Request body
    if let Some(ref body) = endpoint.request_body {
        writeln!(out, "\nRequest Body ({}):", sanitize(&body.content_type)).unwrap();

        if !body.options.is_empty() {
            // OneOf/AnyOf body: show variant options
            writeln!(out, "  One of ({} options):", body.options.len()).unwrap();
            for opt in &body.options {
                writeln!(out, "    phyllotaxis schemas {}", sanitize(opt)).unwrap();
            }
        } else {
            render_fields_section(&mut out, &body.fields);
        }
```

Replace the `else` branch only:

```rust
    // Request body
    if let Some(ref body) = endpoint.request_body {
        writeln!(out, "\nRequest Body ({}):", sanitize(&body.content_type)).unwrap();

        if !body.options.is_empty() {
            // OneOf/AnyOf body: show variant options
            writeln!(out, "  One of ({} options):", body.options.len()).unwrap();
            for opt in &body.options {
                writeln!(out, "    phyllotaxis schemas {}", sanitize(opt)).unwrap();
            }
        } else if body.fields.is_empty() {
            out.push_str("  Raw body (no schema)\n");
        } else {
            render_fields_section(&mut out, &body.fields);
        }
```

### Step 4: Run test to verify it passes

Run: `cargo test test_raw_body_shown_for_csv_content_type -- --nocapture`
Expected: PASS

Also run: `cargo test test_resources_endpoint_post -- --nocapture`
Expected: PASS (regression check — petstore POST /pets still shows fields, not "Raw body")

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/render/text.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: show 'Raw body (no schema)' for non-schema content types (#1)"
```

---

## Task 2: Exclusive Min/Max Constraint Formatting

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs` (`extract_constraints` function)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `openapiv3` 2.2 crate represents OAS 3.0 exclusive bounds as `exclusive_minimum: bool` and `exclusive_maximum: bool` on both `IntegerType` and `NumberType` (the field is a plain `bool`, not `Option<bool>`). When `exclusive_minimum: true` and `minimum: Some(0)`, we want `>0`. When `exclusive_maximum: true` and `maximum: Some(400)`, we want `<400`.

The current code in `extract_constraints` (resources.rs lines 239-252) appends the flag as a separate label: `min:0 exclusiveMinimum`. The fix combines them into a single operator token: `>0`.

Verified fixture data:
- `GeoLocation.accuracy_m`: `minimum: 0, exclusiveMinimum: true` → should show `>0`
- `Bird.wingspan_cm`: `minimum: 1.0, maximum: 400.0, exclusiveMaximum: true` → should show `min:1 <400`

**Success Criteria:**
- [ ] `GeoLocation.accuracy_m` shows `>0` instead of `min:0 exclusiveMinimum`
- [ ] `Bird.wingspan_cm` shows `min:1 <400` instead of `min:1 max:400 exclusiveMaximum`
- [ ] Regular `min:`/`max:` constraints on non-exclusive fields are unchanged

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_exclusive_minimum_formatted_as_operator() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "GeoLocation"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains(">0"),
        "accuracy_m should show >0 for exclusiveMinimum, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        !stdout.contains("exclusiveMinimum"),
        "exclusiveMinimum label must not appear in output, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_exclusive_maximum_formatted_as_operator() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["schemas", "Bird"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("<400"),
        "wingspan_cm should show <400 for exclusiveMaximum, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        !stdout.contains("exclusiveMaximum"),
        "exclusiveMaximum label must not appear in output, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test test_exclusive_minimum_formatted_as_operator test_exclusive_maximum_formatted_as_operator -- --nocapture`
Expected: FAIL

### Step 3: Write minimal implementation

In `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`, replace the `extract_constraints` function. The current version (lines 231-265) appends `exclusiveMinimum` and `exclusiveMaximum` as standalone labels after the min/max values. Replace the entire function:

```rust
fn extract_constraints(kind: &openapiv3::SchemaKind) -> Vec<String> {
    let mut c = Vec::new();
    match kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => {
            if let Some(min) = s.min_length { c.push(format!("min:{}", min)); }
            if let Some(max) = s.max_length { c.push(format!("max:{}", max)); }
            if let Some(ref pat) = s.pattern { c.push(format!("pattern:{}", pat)); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(i)) => {
            if let Some(min) = i.minimum {
                if i.exclusive_minimum {
                    c.push(format!(">{}", min));
                } else {
                    c.push(format!("min:{}", min));
                }
            }
            if let Some(max) = i.maximum {
                if i.exclusive_maximum {
                    c.push(format!("<{}", max));
                } else {
                    c.push(format!("max:{}", max));
                }
            }
            if let Some(mo) = i.multiple_of { c.push(format!("multipleOf:{}", mo)); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(n)) => {
            if let Some(min) = n.minimum {
                if n.exclusive_minimum {
                    c.push(format!(">{}", min));
                } else {
                    c.push(format!("min:{}", min));
                }
            }
            if let Some(max) = n.maximum {
                if n.exclusive_maximum {
                    c.push(format!("<{}", max));
                } else {
                    c.push(format!("max:{}", max));
                }
            }
            if let Some(mo) = n.multiple_of { c.push(format!("multipleOf:{}", mo)); }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Array(a)) => {
            if let Some(min) = a.min_items { c.push(format!("minItems:{}", min)); }
            if let Some(max) = a.max_items { c.push(format!("maxItems:{}", max)); }
            if a.unique_items { c.push("uniqueItems".to_string()); }
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

Note: `exclusive_minimum` and `exclusive_maximum` are plain `bool` fields on `IntegerType` and `NumberType` in openapiv3 2.2 — no `Option` unwrapping needed. `unique_items` on `ArrayType` is also a plain `bool`, so the existing `if a.unique_items` check is already correct and unchanged.

### Step 4: Run tests to verify they pass

Run: `cargo test test_exclusive_minimum_formatted_as_operator test_exclusive_maximum_formatted_as_operator -- --nocapture`
Expected: PASS

Also run: `cargo test test_constraints_integer -- --nocapture`
Expected: PASS (regression — Settings.max_upload_size_mb uses regular min:/max:, not exclusive)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: format exclusive bounds as operators (>0, <400) instead of labels (#2)"
```

---

## Task 3: Array Item Types Propagate to Type Display

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs` (`format_type_display` function)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `openapiv3` 2.2 crate defines `ArrayType.items` as `Option<ReferenceOr<Box<Schema>>>`. The current `format_type_display` handles `ReferenceOr::Reference` (extracts the ref name → `TreeNode[]`) but falls through to `"array"` for `ReferenceOr::Item(boxed)`. The fix adds a branch that recursively calls `format_type_display(&boxed.schema_kind)` — `Box<Schema>` auto-derefs, so `.schema_kind` works directly.

Verified fixture data: `POST /files/upload-batch` has a `files` field with `type: array, items: {type: string, format: binary}` — should show `binary[]`.

**Success Criteria:**
- [ ] `POST /files/upload-batch` shows `binary[]` for the `files` field, not `array`
- [ ] `$ref`-based arrays like `TreeNode[]` are unaffected

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_array_item_type_propagates_for_inline_binary() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["resources", "files", "POST", "/files/upload-batch"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("binary[]"),
        "files field should show binary[], got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_array_item_type_propagates_for_inline_binary -- --nocapture`
Expected: FAIL (shows `array` instead of `binary[]`)

### Step 3: Write minimal implementation

In `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`, find the `Array` arm in `format_type_display`. Current code (lines 287-294):

```rust
            openapiv3::Type::Array(arr) => match &arr.items {
                Some(openapiv3::ReferenceOr::Reference { reference }) => {
                    let name =
                        spec::schema_name_from_ref(reference.as_str()).unwrap_or("object");
                    format!("{}[]", name)
                }
                _ => "array".to_string(),
            },
```

Replace with:

```rust
            openapiv3::Type::Array(arr) => match &arr.items {
                Some(openapiv3::ReferenceOr::Reference { reference }) => {
                    let name =
                        spec::schema_name_from_ref(reference.as_str()).unwrap_or("object");
                    format!("{}[]", name)
                }
                Some(openapiv3::ReferenceOr::Item(boxed)) => {
                    let item_type = format_type_display(&boxed.schema_kind);
                    format!("{}[]", item_type)
                }
                _ => "array".to_string(),
            },
```

`boxed` is `Box<Schema>`. `boxed.schema_kind` auto-derefs through `Box` to access the inner `Schema`'s field. No explicit `*` deref or `.as_ref()` needed.

### Step 4: Run test to verify it passes

Run: `cargo test test_array_item_type_propagates_for_inline_binary -- --nocapture`
Expected: PASS

Also run: `cargo test test_build_fields_allof -- --nocapture`
Expected: PASS (regression — tags field in PetList uses `$ref` items, still works)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: propagate inline array item types (binary[], string[]) in type display (#3)"
```

---

## Task 4: Trailing Whitespace on Empty Header Descriptions

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (response headers section in `render_endpoint_detail`)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The header row is formatted as `"      {}  {}  {}"` with name, type, and description. When `description` is `None`, the empty string still produces trailing spaces. The fix omits the description column entirely when there is no description.

**Success Criteria:**
- [ ] `HEAD /health` response headers (`X-Health-Status`) produce no trailing whitespace in each header line
- [ ] Headers with descriptions still show them

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_no_trailing_whitespace_on_empty_header_description() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["resources", "health", "HEAD", "/health"]);
    assert_eq!(code, 0);
    // X-Health-Status has no description — its line must not end in spaces
    let header_line = stdout
        .lines()
        .find(|l| l.contains("X-Health-Status"))
        .expect("X-Health-Status header line not found");
    assert!(
        !header_line.ends_with(' '),
        "Header line must not have trailing whitespace, got: {:?}",
        header_line
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_no_trailing_whitespace_on_empty_header_description -- --nocapture`
Expected: FAIL (line ends with trailing spaces)

### Step 3: Write minimal implementation

In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`, find the response headers block inside `render_endpoint_detail`. Current code (around line 188):

```rust
            if !resp.headers.is_empty() {
                out.push_str("    Headers:\n");
                for h in &resp.headers {
                    let desc = sanitize(h.description.as_deref().unwrap_or(""));
                    writeln!(out, "      {}  {}  {}", sanitize(&h.name), sanitize(&h.type_display), desc).unwrap();
                }
            }
```

Replace with:

```rust
            if !resp.headers.is_empty() {
                out.push_str("    Headers:\n");
                for h in &resp.headers {
                    match h.description.as_deref() {
                        Some(desc) if !desc.is_empty() => {
                            writeln!(out, "      {}  {}  {}", sanitize(&h.name), sanitize(&h.type_display), sanitize(desc)).unwrap();
                        }
                        _ => {
                            writeln!(out, "      {}  {}", sanitize(&h.name), sanitize(&h.type_display)).unwrap();
                        }
                    }
                }
            }
```

### Step 4: Run test to verify it passes

Run: `cargo test test_no_trailing_whitespace_on_empty_header_description -- --nocapture`
Expected: PASS

Also run: `cargo test test_render_response_headers -- --nocapture`
Expected: PASS (unit test with a header that has a description — still shows it)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/render/text.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: omit trailing whitespace when response header has no description (#4)"
```

---

## Task 5: Remove Top-Level Links Duplication in JSON Output

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs` — add `#[serde(skip_serializing)]` on the `Endpoint.links` field
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs` — remove the `assert!(v["links"].is_array(), ...)` assertion from an existing unit test
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `Endpoint` struct's `links` field is an aggregation of all per-response links. In JSON output, links already appear on each response object where they're defined. The top-level `links` is redundant. The text renderer reads `endpoint.links` directly by name (`if !endpoint.links.is_empty()` at text.rs line 205) and must keep it. Using `#[serde(skip_serializing)]` removes the field from JSON serialization while leaving the in-memory field intact and accessible to the text renderer.

The existing unit test `test_endpoint_json_includes_new_fields` in `src/render/json.rs` (line 634) asserts `v["links"].is_array()`. This assertion must be removed since we are intentionally dropping the field from JSON output.

**Success Criteria:**
- [ ] `POST /users --json` output does not contain a top-level `"links"` key
- [ ] `POST /users --json` responses still contain per-response links
- [ ] Text output for `POST /users` still shows the Links section

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_json_endpoint_no_top_level_links() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["--json", "resources", "users", "POST", "/users"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", &stdout[..200.min(stdout.len())]));
    assert!(
        json.get("links").is_none(),
        "Top-level 'links' must not appear in JSON endpoint detail, got: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );
    // Per-response links should still be present
    let responses = json["responses"].as_array().expect("responses array");
    let has_response_links = responses.iter().any(|r| {
        r.get("links").and_then(|l| l.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
    });
    assert!(has_response_links, "Per-response links must still appear in JSON");
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_json_endpoint_no_top_level_links -- --nocapture`
Expected: FAIL (top-level `"links"` is present)

### Step 3: Write minimal implementation

**In `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs`**, modify the `Endpoint` struct. Current (lines 12-27):

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
    pub callbacks: Vec<CallbackEntry>,
    pub links: Vec<ResponseLink>,
    pub drill_deeper: Vec<String>,
}
```

Replace with:

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
    pub callbacks: Vec<CallbackEntry>,
    #[serde(skip_serializing)]
    pub links: Vec<ResponseLink>,
    pub drill_deeper: Vec<String>,
}
```

**In `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`**, remove the `links` assertion from `test_endpoint_json_includes_new_fields`. Current (line 634):

```rust
        assert!(v["callbacks"].is_array(), "callbacks should be present as array");
        assert!(v["links"].is_array(), "links should be present as array");
```

Replace with:

```rust
        assert!(v["callbacks"].is_array(), "callbacks should be present as array");
```

Remove only the `links` assertion line. The `callbacks` assertion stays.

### Step 4: Run test to verify it passes

Run: `cargo test test_json_endpoint_no_top_level_links -- --nocapture`
Expected: PASS

Also run: `cargo test test_render_links_section -- --nocapture`
Expected: PASS (text renderer still reads `endpoint.links` from memory — the field is in-memory, just not serialized)

Also run: `cargo test test_endpoint_json_includes_new_fields -- --nocapture`
Expected: PASS (updated assertion — no longer checks for `links`)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/models/resource.rs \
        /home/hhewett/.local/src/phyllotaxis/src/render/json.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: remove top-level links from JSON endpoint output; links stay on responses (#5)"
```

---

## Task 6: Callback Operation Count in List View

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (`render_callback_list` function)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `CallbackEntry` struct already has `operations: Vec<CallbackOperation>` — no extraction change needed, just render it. The kitchen-sink fixture has two callbacks: `onEvent` (1 POST operation) and `onStatusChange` (1 POST operation), both defined on `POST /notifications/subscribe`.

**Success Criteria:**
- [ ] `phyllotaxis callbacks` output shows `(1 operation)` after `onEvent`
- [ ] Plural form: `(2 operations)` for a callback with 2 operations

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_callback_list_shows_operation_count() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["callbacks"]);
    assert_eq!(code, 0);
    // onEvent has 1 operation (POST)
    assert!(
        stdout.contains("(1 operation)"),
        "Callback list must show operation count, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_callback_list_shows_operation_count -- --nocapture`
Expected: FAIL

### Step 3: Write minimal implementation

In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`, find `render_callback_list` (line 561). Current `for cb in callbacks` loop:

```rust
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
```

Replace the `for cb in callbacks` loop with:

```rust
    writeln!(out, "Callbacks ({} total):", callbacks.len()).unwrap();
    for cb in callbacks {
        let op_count = cb.operations.len();
        let op_label = if op_count == 1 { "operation" } else { "operations" };
        writeln!(
            out,
            "  {}  (on {} {})  ({} {})",
            sanitize(&cb.name),
            sanitize(&cb.defined_on_method),
            sanitize(&cb.defined_on_path),
            op_count,
            op_label
        ).unwrap();
    }
```

### Step 4: Run test to verify it passes

Run: `cargo test test_callback_list_shows_operation_count -- --nocapture`
Expected: PASS

Also run: `cargo test test_render_callback_list -- --nocapture`
Expected: PASS (the unit test in text.rs only checks for "onEvent" and the drill hint — it still passes because those strings are still present)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/render/text.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: show operation count per callback in callback list view (#6)"
```

---

## Task 7: Verify --expand Flag Works on Endpoint Detail View

**Files:**
- Test only: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `--expand` flag is already fully implemented and wired. Verified in `src/main.rs`:
- Lines 17-19: `#[arg(long, global = true)] expand: bool,` — defined as a global flag on `Cli`
- Line 140: `commands::resources::get_endpoint_detail(&loaded.api, method, path, cli.expand)` — wired through

The design doc issue ("CLI doesn't expose `--expand` on resources subcommand") was based on a misreading. The global flag `expand: bool` on `Cli` is accessible anywhere in the CLI by clap's global flag mechanism — it works after any subcommand argument, including `resources pets POST /pets --expand`. No implementation change is needed.

This task is a test-only verification that the existing behavior works correctly.

**Success Criteria:**
- [ ] `phyllotaxis resources pets POST /pets --expand` exits 0 and shows `Owner:` with nested fields inline

### Step 1: Write the test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_resources_endpoint_expand_flag() {
    let (stdout, _stderr, code) =
        run_with_petstore(&["resources", "pets", "POST", "/pets", "--expand"]);
    assert_eq!(code, 0, "Expected exit code 0, got stderr: {}", _stderr);
    // With --expand, Owner's nested fields should appear inline
    assert!(
        stdout.contains("Owner"),
        "Expected Owner reference in expanded output, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    // When expanded, the nested type is shown with a colon: "owner  Owner:"
    assert!(
        stdout.contains("Owner:"),
        "With --expand, owner field should show 'Owner:' with nested fields, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run test to verify it passes

Run: `cargo test test_resources_endpoint_expand_flag -- --nocapture`
Expected: PASS (the flag is already wired — this test should pass immediately)

If it fails for any reason, verify manually:
```bash
cargo run -- --spec tests/fixtures/petstore.yaml resources pets POST /pets --expand
```

### Step 3: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "test: verify --expand flag works on resources endpoint view (#7)"
```

---

## Task 8: Search Covers Callbacks

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs` (`SearchResults` struct and `search` function)
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (`render_search` function)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** The `SearchResults` struct needs a new `callbacks` field. The `search` function must query `list_all_callbacks`. The text renderer must display the new section. The JSON renderer calls `serialize(results, is_tty)` — since `SearchResults` derives `serde::Serialize`, adding the field automatically includes it in JSON output.

**Success Criteria:**
- [ ] `phyllotaxis search onEvent` returns results with a Callbacks section
- [ ] `phyllotaxis search callback` returns results including callback-related matches
- [ ] Existing search results for resources/endpoints/schemas are unaffected

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_search_finds_callbacks() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["search", "onEvent"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Callbacks:"),
        "Search must include a Callbacks section, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("onEvent"),
        "Search for 'onEvent' must find the callback, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test test_search_finds_callbacks -- --nocapture`
Expected: FAIL

### Step 3: Write minimal implementation

**In `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs`:**

Add `CallbackMatch` struct and `callbacks` field to `SearchResults`. Current `SearchResults` (lines 5-11):

```rust
#[derive(Debug, serde::Serialize)]
pub struct SearchResults {
    pub term: String,
    pub resources: Vec<ResourceMatch>,
    pub endpoints: Vec<EndpointMatch>,
    pub schemas: Vec<SchemaMatch>,
}
```

Replace with:

```rust
#[derive(Debug, serde::Serialize)]
pub struct CallbackMatch {
    pub name: String,
    pub defined_on_path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchResults {
    pub term: String,
    pub resources: Vec<ResourceMatch>,
    pub endpoints: Vec<EndpointMatch>,
    pub schemas: Vec<SchemaMatch>,
    pub callbacks: Vec<CallbackMatch>,
}
```

Add the callback search just before the final `SearchResults { ... }` construction at the bottom of the `search` function. Current construction (lines 133-138):

```rust
    SearchResults {
        term: term.to_string(),
        resources,
        endpoints,
        schemas,
    }
```

Replace with:

```rust
    // Search callbacks
    let all_callbacks = crate::commands::callbacks::list_all_callbacks(api);
    let callbacks: Vec<CallbackMatch> = all_callbacks
        .into_iter()
        .filter(|cb| {
            cb.name.to_lowercase().contains(&term_lower)
                || cb.defined_on_path.to_lowercase().contains(&term_lower)
        })
        .map(|cb| CallbackMatch {
            name: cb.name,
            defined_on_path: cb.defined_on_path,
        })
        .collect();

    SearchResults {
        term: term.to_string(),
        resources,
        endpoints,
        schemas,
        callbacks,
    }
```

**In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`:**

Update `render_search` in two places.

First, update the `has_any` check. Current code (lines 626-628):

```rust
    let has_any = !results.resources.is_empty()
        || !results.endpoints.is_empty()
        || !results.schemas.is_empty();
```

Replace with:

```rust
    let has_any = !results.resources.is_empty()
        || !results.endpoints.is_empty()
        || !results.schemas.is_empty()
        || !results.callbacks.is_empty();
```

Second, add the callbacks section. Insert it after the schemas section and before the drill_deeper block. The schemas section ends and the drill_deeper block begins here (lines 668-676):

```rust
    if !results.schemas.is_empty() {
        out.push_str("\nSchemas:\n");
        for s in &results.schemas {
            writeln!(out, "  {}", sanitize(&s.name)).unwrap();
        }
    }

    // Drill deeper (TTY only)
    if is_tty {
```

Insert the callbacks section between those two blocks:

```rust
    if !results.schemas.is_empty() {
        out.push_str("\nSchemas:\n");
        for s in &results.schemas {
            writeln!(out, "  {}", sanitize(&s.name)).unwrap();
        }
    }

    if !results.callbacks.is_empty() {
        out.push_str("\nCallbacks:\n");
        for cb in &results.callbacks {
            writeln!(
                out,
                "  {}  (on {})",
                sanitize(&cb.name),
                sanitize(&cb.defined_on_path)
            ).unwrap();
            if is_tty {
                writeln!(out, "    phyllotaxis callbacks {}", sanitize(&cb.name)).unwrap();
            }
        }
    }

    // Drill deeper (TTY only)
    if is_tty {
```

**Also update the existing unit tests in text.rs and json.rs that construct `SearchResults` directly.** Search for `SearchResults {` in both files:

In `src/render/text.rs`, there are several unit tests constructing `SearchResults` (around lines 1334, 1358, 1382). Each one must add `callbacks: vec![]`. For example:

```rust
        let results = SearchResults {
            term: "pets".to_string(),
            resources: vec![],
            endpoints: vec![EndpointMatch { ... }],
            schemas: vec![],
            callbacks: vec![],  // add this line
        };
```

In `src/render/json.rs`, the `test_all_json_outputs_parse` test constructs `SearchResults` (around line 452). Add `callbacks: vec![]`:

```rust
        let results = SearchResults {
            term: "test".to_string(),
            resources: vec![],
            endpoints: vec![],
            schemas: vec![],
            callbacks: vec![],  // add this line
        };
```

### Step 4: Run test to verify it passes

Run: `cargo test test_search_finds_callbacks -- --nocapture`
Expected: PASS

Also run: `cargo test test_search_pet test_search_no_results -- --nocapture`
Expected: PASS (regression — existing search tests unaffected)

Also run: `cargo test test_render_search -- --nocapture`
Expected: PASS (unit tests in text.rs — all `SearchResults` constructions now have `callbacks: vec![]`)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/commands/search.rs \
        /home/hhewett/.local/src/phyllotaxis/src/render/text.rs \
        /home/hhewett/.local/src/phyllotaxis/src/render/json.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: include callbacks in search results (#8)"
```

---

## Task 9: Fuzzy Matching for Callback Names

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/commands/callbacks.rs` (add `suggest_similar_callbacks`)
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/main.rs` (wire suggestions into callbacks not-found error)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None

**Background:** `strsim = "0.11"` is confirmed in `Cargo.toml`. The existing `suggest_similar` function in `resources.rs` (lines 811-819) uses `strsim::jaro_winkler` without any `use` import at the top of the file — it calls the full crate path directly. The new `suggest_similar_callbacks` in `callbacks.rs` follows the same pattern.

Existing `suggest_similar` in `resources.rs` for comparison:

```rust
pub fn suggest_similar<'a>(groups: &'a [ResourceGroup], slug: &str) -> Vec<&'a str> {
    let slug_lower = slug.to_lowercase();
    groups
        .iter()
        .filter(|g| strsim::jaro_winkler(&slug_lower, &g.slug.to_lowercase()) > 0.8)
        .take(3)
        .map(|g| g.slug.as_str())
        .collect()
}
```

The not-found error path in `main.rs` (lines 278-285) currently:

```rust
                        None => {
                            if cli.json {
                                eprintln!("{}", json_error(&format!("Callback '{}' not found.", name)));
                            } else {
                                eprintln!("Error: Callback '{}' not found.", name);
                            }
                            std::process::exit(1);
                        }
```

**Success Criteria:**
- [ ] `phyllotaxis callbacks onEven` (missing 't') suggests `onEvent` in stderr
- [ ] Non-matching typos like `phyllotaxis callbacks xyzzy` give no suggestions

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_callbacks_fuzzy_suggestion_on_typo() {
    let (_stdout, stderr, code) = run_with_kitchen_sink(&["callbacks", "onEven"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("onEvent"),
        "Expected suggestion 'onEvent' for typo 'onEven', got:\n{}",
        stderr
    );
}

#[test]
fn test_callbacks_no_suggestion_for_nonsense() {
    let (_stdout, stderr, code) = run_with_kitchen_sink(&["callbacks", "xyzzy"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("not found"),
        "Expected not-found message, got:\n{}",
        stderr
    );
    // "Did you mean:" must not appear — no close match
    assert!(
        !stderr.contains("Did you mean"),
        "Must not suggest for completely different name, got:\n{}",
        stderr
    );
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test test_callbacks_fuzzy_suggestion_on_typo test_callbacks_no_suggestion_for_nonsense -- --nocapture`
Expected: FAIL (first test: no suggestion shown; second may pass already since the not-found path exists)

### Step 3: Write minimal implementation

**In `/home/hhewett/.local/src/phyllotaxis/src/commands/callbacks.rs`**, add after `find_callback` (after line 129):

```rust
/// Returns up to 3 callback names that are similar to `name` using Jaro-Winkler distance.
/// Mirrors the `suggest_similar` pattern from commands/resources.rs.
pub fn suggest_similar_callbacks<'a>(all: &'a [CallbackEntry], name: &str) -> Vec<&'a str> {
    let name_lower = name.to_lowercase();
    all.iter()
        .filter(|cb| strsim::jaro_winkler(&name_lower, &cb.name.to_lowercase()) > 0.8)
        .take(3)
        .map(|cb| cb.name.as_str())
        .collect()
}
```

No `use strsim;` import needed — the same pattern as `resources.rs` which also calls `strsim::jaro_winkler` via full path.

**In `/home/hhewett/.local/src/phyllotaxis/src/main.rs`**, replace the callback not-found error block. Current (lines 278-285):

```rust
                        None => {
                            if cli.json {
                                eprintln!("{}", json_error(&format!("Callback '{}' not found.", name)));
                            } else {
                                eprintln!("Error: Callback '{}' not found.", name);
                            }
                            std::process::exit(1);
                        }
```

Replace with:

```rust
                        None => {
                            if cli.json {
                                eprintln!("{}", json_error(&format!("Callback '{}' not found.", name)));
                            } else {
                                eprintln!("Error: Callback '{}' not found.", name);
                                let suggestions = commands::callbacks::suggest_similar_callbacks(&callbacks, name);
                                if !suggestions.is_empty() {
                                    eprintln!("Did you mean:");
                                    for s in &suggestions {
                                        eprintln!("  phyllotaxis callbacks {}", s);
                                    }
                                }
                            }
                            std::process::exit(1);
                        }
```

Note: `callbacks` (the `Vec<CallbackEntry>`) is already in scope at this point — it was bound on line 258: `let callbacks = commands::callbacks::list_all_callbacks(&loaded.api);`. The `find_callback` call on line 269 also calls `list_all_callbacks` internally. Both calls iterate the spec, but this is acceptable given the current architecture.

### Step 4: Run tests to verify they pass

Run: `cargo test test_callbacks_fuzzy_suggestion_on_typo test_callbacks_no_suggestion_for_nonsense -- --nocapture`
Expected: PASS

Also run: `cargo test test_find_callback_not_found -- --nocapture`
Expected: PASS (existing unit test — find_callback returns None for nonexistent, which is unchanged)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/commands/callbacks.rs \
        /home/hhewett/.local/src/phyllotaxis/src/main.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: suggest similar callback names on not-found using Jaro-Winkler (#9)"
```

---

## Task 10: Callback Count in Overview

**Files:**
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/commands/overview.rs` (add `callback_count` to `OverviewData`, populate in `build`)
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` (`render_overview` — display callback count, fix 4 unit test `OverviewData` constructions)
- Modify: `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs` (`render_overview` — include callback count, fix 1 unit test `OverviewData` construction)
- Test: `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

**Depends on:** None (self-contained struct change)

**All `OverviewData { ... }` constructions that need `callback_count: 0` added:**

From reading the actual source:

`src/render/text.rs` — 4 unit tests:
- `test_render_overview_basic` (line 828): `OverviewData { title: "Petstore API", ... schema_count: 4, }` — add `callback_count: 0`
- `test_render_overview_no_auth` (line 853): `OverviewData { title: "Test", ... schema_count: 0, }` — add `callback_count: 0`
- `test_render_overview_with_description` (line 868): `OverviewData { title: "Test", ... schema_count: 0, }` — add `callback_count: 0`
- `test_render_overview_with_variables` (line 883): `OverviewData { title: "Test", ... schema_count: 0, }` — add `callback_count: 0`

`src/render/json.rs` — 1 unit test:
- `test_all_json_outputs_parse` (line 391): `OverviewData { title: "Test API", ... schema_count: 0, }` — add `callback_count: 0`

**Success Criteria:**
- [ ] Overview text output includes `phyllotaxis callbacks    List all webhook callbacks (2 available)` for kitchen-sink
- [ ] Overview JSON includes `"callback_count": 2` for kitchen-sink
- [ ] Petstore overview shows `callback_count: 0` (petstore has no callbacks)

### Step 1: Write the failing test

```rust
// In tests/integration_tests.rs:

#[test]
fn test_overview_shows_callback_count() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&[]);
    assert_eq!(code, 0);
    // The callbacks line should include the count (kitchen-sink has 2 callbacks)
    assert!(
        stdout.contains("2 available") || stdout.contains("(2"),
        "Overview must include callback count, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_overview_json_includes_callback_count() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--json"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", &stdout[..200.min(stdout.len())]));
    assert!(
        json.get("callback_count").is_some(),
        "JSON overview must include 'callback_count', got: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );
    assert_eq!(
        json["callback_count"], 2,
        "Kitchen-sink has 2 callbacks (onEvent, onStatusChange)"
    );
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test test_overview_shows_callback_count test_overview_json_includes_callback_count -- --nocapture`
Expected: FAIL

### Step 3: Write minimal implementation

**In `/home/hhewett/.local/src/phyllotaxis/src/commands/overview.rs`:**

Add `callback_count` to `OverviewData`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct OverviewData {
    pub title: String,
    pub description: Option<String>,
    pub base_urls: Vec<String>,
    pub server_variables: Vec<ServerVar>,
    pub auth_schemes: Vec<String>,
    pub resource_count: usize,
    pub schema_count: usize,
    pub callback_count: usize,
}
```

In the `build` function, after the `schema_count` line, add:

```rust
    let callback_count = crate::commands::callbacks::list_all_callbacks(&loaded.api).len();
```

Update the `OverviewData { ... }` construction at the bottom to include `callback_count`.

**In `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`:**

Update `render_overview` to show callback count. Current line 77:

```rust
    out.push_str("  phyllotaxis callbacks    List all webhook callbacks\n");
```

Replace with:

```rust
    writeln!(
        out,
        "  phyllotaxis callbacks    List all webhook callbacks ({} available)",
        data.callback_count
    ).unwrap();
```

Also fix all 4 unit tests in text.rs that construct `OverviewData` directly. Each test struct literal ends at `schema_count: N,` — add `callback_count: 0` after it:

- `test_render_overview_basic` (~line 836): change `schema_count: 4,` to `schema_count: 4, callback_count: 0,`
- `test_render_overview_no_auth` (~line 860): change `schema_count: 0,` to `schema_count: 0, callback_count: 0,`
- `test_render_overview_with_description` (~line 876): change `schema_count: 0,` to `schema_count: 0, callback_count: 0,`
- `test_render_overview_with_variables` (~line 895): change `schema_count: 0,` to `schema_count: 0, callback_count: 0,`

**In `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`:**

Update `render_overview`'s `OverviewJson` struct to include `callback_count`. Current struct definition (lines 92-101):

```rust
    #[derive(serde::Serialize)]
    struct OverviewJson<'a> {
        title: &'a str,
        description: Option<&'a str>,
        servers: Vec<ServerJson<'a>>,
        auth: &'a [String],
        resource_count: usize,
        schema_count: usize,
        commands: CommandsJson,
    }
```

Replace with:

```rust
    #[derive(serde::Serialize)]
    struct OverviewJson<'a> {
        title: &'a str,
        description: Option<&'a str>,
        servers: Vec<ServerJson<'a>>,
        auth: &'a [String],
        resource_count: usize,
        schema_count: usize,
        callback_count: usize,
        commands: CommandsJson,
    }
```

Update the `OverviewJson { ... }` construction (around line 135) to include `callback_count: data.callback_count`.

Also fix the unit test in json.rs (`test_all_json_outputs_parse`, line 391) that constructs `OverviewData`. Change `schema_count: 0,` to `schema_count: 0, callback_count: 0,`.

### Step 4: Run tests to verify they pass

Run: `cargo test test_overview_shows_callback_count test_overview_json_includes_callback_count -- --nocapture`
Expected: PASS

Also run: `cargo test test_render_overview_basic test_render_overview_no_auth -- --nocapture`
Expected: PASS (these unit tests now have `callback_count: 0`)

Also run: `cargo test test_overview_text test_overview_json -- --nocapture`
Expected: PASS (petstore integration tests — petstore has no callbacks, count is 0)

### Step 5: Commit

```bash
git add /home/hhewett/.local/src/phyllotaxis/src/commands/overview.rs \
        /home/hhewett/.local/src/phyllotaxis/src/render/text.rs \
        /home/hhewett/.local/src/phyllotaxis/src/render/json.rs \
        /home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs
git commit -m "fix: add callback count to overview data, text, and JSON output (#10)"
```

---

## Final Verification

After all 10 tasks are complete:

```bash
cargo test -- --nocapture 2>&1 | tail -20
```

Expected: All tests pass, no regressions.

Manual spot checks:
```bash
SPEC=tests/fixtures/kitchen-sink.yaml

# #1 — CSV body
cargo run -- --spec $SPEC resources admin POST /admin/bulk-import

# #2 — Exclusive bounds
cargo run -- --spec $SPEC schemas GeoLocation

# #3 — Array item types
cargo run -- --spec $SPEC resources files POST /files/upload-batch

# #4 — No trailing whitespace
cargo run -- --spec $SPEC resources health HEAD /health | cat -A | grep X-Health

# #5 — No top-level links in JSON
cargo run -- --spec $SPEC --json resources users POST /users | python3 -c "import sys,json; d=json.load(sys.stdin); print('links' in d)"

# #6 — Callback operation count
cargo run -- --spec $SPEC callbacks

# #7 — expand on resources
cargo run -- --spec tests/fixtures/petstore.yaml resources pets POST /pets --expand

# #8 — Search for callbacks
cargo run -- --spec $SPEC search onEvent

# #9 — Fuzzy suggestion
cargo run -- --spec $SPEC callbacks onEven

# #10 — Callback count in overview
cargo run -- --spec $SPEC
```
