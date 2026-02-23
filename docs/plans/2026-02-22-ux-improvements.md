# Plan: UX Improvements
**Date:** 2026-02-22

Four improvements identified from e2e testing against the Aembit cloud and edge specs.

---

## Preamble: What the codebase review revealed

**Search results already have `resource_slug`.** `EndpointMatch` in `src/commands/search.rs` has `pub resource_slug: String` and it is already populated. The gap is purely on the render side — the text renderer doesn't emit the drill-down command, and the JSON renderer already includes it via derive.

**Endpoint drill-deeper hints are fully absent.** The `Endpoint` struct has no `drill_deeper` field. `render_endpoint_detail` in `text.rs` emits no "Drill deeper:" block. `render_endpoint_detail` in `json.rs` just calls `serialize(endpoint, is_tty)` on the raw struct.

**`PHYLLOTAXIS_SPEC` env var.** The env var check belongs inside `resolve_spec_path`, between the `--spec` flag block and the config-file block. `main.rs` does not need to change.

---

## Dependency graph

```
Task 1 (Endpoint model: add drill_deeper field)
  └─ Task 2 (Populate drill_deeper in get_endpoint_detail)
       ├─ Task 3 (text renderer: emit drill_deeper)
       └─ Task 4 (JSON renderer: assert drill_deeper)

Task 5 (Search text renderer: emit full commands)    [independent]
Task 6 (PHYLLOTAXIS_SPEC env var)                    [independent]
```

Tasks 5 and 6 are independent of the endpoint drill-deeper chain and can run in parallel with any of Tasks 1–4.

---

## Task 1 — Add `drill_deeper` to `Endpoint` struct

**File:** `src/models/resource.rs`

Add one field to `Endpoint`:

```rust
pub struct Endpoint {
    // ... existing fields unchanged ...
    pub security_schemes: Vec<String>,
    pub drill_deeper: Vec<String>,   // fully-formed CLI commands
}
```

Also add `schema_ref: Option<String>` to `RequestBody` to preserve the top-level schema name for concrete (non-oneOf) request bodies:

```rust
pub struct RequestBody {
    pub content_type: String,
    pub fields: Vec<Field>,
    pub options: Vec<String>,           // oneOf/anyOf variant names
    pub schema_ref: Option<String>,     // top-level $ref name for concrete bodies
    pub example: Option<serde_json::Value>,
}
```

**Impact:** All `Endpoint { ... }` and `RequestBody { ... }` struct literals in tests must add the new fields. The compiler enforces this.

**Struct literals to update:**
- `src/commands/resources.rs` — `extract_resource_groups` function (~line 56): add `drill_deeper: vec![]`
- `src/render/text.rs` tests — `Endpoint` literals: add `drill_deeper: vec![]`; `RequestBody` literals: add `schema_ref: None`
- `src/render/json.rs` tests — `Endpoint` literal: add `drill_deeper: vec![]`

**TDD:** Run `cargo test` after adding the fields — it will fail to compile at every literal site. Fix each site. Green = done.

---

## Task 2 — Populate `drill_deeper` in `get_endpoint_detail`

**File:** `src/commands/resources.rs`

### Step 1 — Update `extract_request_body`

In `extract_request_body`, when the request body resolves to a concrete `$ref` (not oneOf/anyOf), capture the schema name in `RequestBody.schema_ref`. For the oneOf/anyOf branch, `schema_ref` stays `None` (the names are already in `options`).

**Implementation note:** The current code (line 439–449) matches `schema_ref` but the `Reference` arm resolves to a `&Schema` without retaining the name string. This step requires restructuring that arm to bind the name before resolving the schema. Save `sname.to_string()` as an `Option<String>` before the match, then pass it to `RequestBody.schema_ref`. The `Item` arm (inline schema, no `$ref`) sets `schema_ref: None`.

```rust
// Extract the top-level schema name if present (for concrete $ref bodies)
let top_ref_name: Option<String> = match schema_ref {
    openapiv3::ReferenceOr::Reference { reference } => {
        spec::schema_name_from_ref(reference).map(|s| s.to_string())
    }
    _ => None,
};

// ... (resolve schema as before) ...

// Concrete path: no oneOf/anyOf options
Some(RequestBody {
    content_type: "application/json".to_string(),
    fields,
    options: Vec::new(),
    schema_ref: top_ref_name,   // preserve the resolved name
    example,
})
```

### Step 2 — Compute `drill_deeper` in `get_endpoint_detail`

After computing `responses` and `request_body`, before constructing `Endpoint`:

```rust
// 7. Drill deeper hints
let mut seen = std::collections::HashSet::new();
let mut drill_deeper = Vec::new();

// 2xx response schema refs only
for resp in &responses {
    if resp.status_code.starts_with('2') {
        if let Some(ref name) = resp.schema_ref {
            if seen.insert(name.clone()) {
                drill_deeper.push(format!("phyllotaxis schemas {}", name));
            }
        }
    }
}

// Request body: oneOf/anyOf options OR concrete schema ref
if let Some(ref body) = request_body {
    if !body.options.is_empty() {
        for name in &body.options {
            if seen.insert(name.clone()) {
                drill_deeper.push(format!("phyllotaxis schemas {}", name));
            }
        }
    } else if let Some(ref name) = body.schema_ref {
        if seen.insert(name.clone()) {
            drill_deeper.push(format!("phyllotaxis schemas {}", name));
        }
    }
}
```

### Tests to write (characterize → implement → verify)

Write tests first; they will fail until the logic is in place.

```
test_drill_deeper_2xx_response_schema
  - petstore GET /pets/{id}: 200 response schema_ref "Pet"
  - Assert drill_deeper == ["phyllotaxis schemas Pet"]

test_drill_deeper_excludes_error_responses
  - petstore POST /pets: 201 Pet, 400/409 no schema ref
  - Assert drill_deeper contains only "phyllotaxis schemas Pet"

test_drill_deeper_deduplication
  - Construct synthetic endpoint where same schema appears in 200 and 201
  - Assert schema name appears exactly once

test_drill_deeper_empty_when_no_schemas
  - petstore DELETE /pets/{id}: 204 no content, no request body
  - Assert drill_deeper is empty
```

**Depends on:** Task 1.

---

## Task 3 — Text renderer: emit drill-deeper for endpoint detail

**File:** `src/render/text.rs`, function `render_endpoint_detail`

After the existing "Errors" block, add:

```rust
if is_tty && !endpoint.drill_deeper.is_empty() {
    out.push_str("\nDrill deeper:\n");
    for cmd in &endpoint.drill_deeper {
        writeln!(out, "  {}", sanitize(cmd)).unwrap();
    }
}
```

### Tests to write

```
test_render_endpoint_detail_drill_deeper_shown_on_tty
  - Endpoint with drill_deeper: vec!["phyllotaxis schemas Pet".to_string()]
  - is_tty = true
  - Assert output contains "Drill deeper:" and "phyllotaxis schemas Pet"

test_render_endpoint_detail_drill_deeper_hidden_off_tty
  - Same endpoint, is_tty = false
  - Assert output does NOT contain "Drill deeper:"

test_render_endpoint_detail_no_section_when_empty
  - Endpoint with drill_deeper: vec![]
  - is_tty = true
  - Assert "Drill deeper:" is NOT in output
```

**Depends on:** Task 1.

---

## Task 4 — JSON renderer: assert `drill_deeper` in output

**File:** `src/render/json.rs`

`render_endpoint_detail` is a one-liner (`serialize(endpoint, is_tty)`). Because `Endpoint` derives `serde::Serialize`, the `drill_deeper` field appears automatically once Task 1 is done. No code change needed.

Update the existing test `test_all_json_outputs_parse` to assert the field:

```rust
assert!(v["drill_deeper"].is_array());
```

### Tests to write

```
test_endpoint_detail_json_includes_drill_deeper
  - Endpoint with drill_deeper: vec!["phyllotaxis schemas Pet".to_string()]
  - render_endpoint_detail with is_tty = false
  - Parse JSON
  - Assert v["drill_deeper"] == ["phyllotaxis schemas Pet"]
```

**Depends on:** Task 1.

---

## Task 5 — Search text renderer: emit full drill-down commands

**File:** `src/render/text.rs`, function `render_search`

`EndpointMatch.resource_slug` already exists and is populated. This is a render-only change.

Replace the endpoint rendering block to add the drill-down command on a second line for each result:

```rust
if !results.endpoints.is_empty() {
    out.push_str("\nEndpoints:\n");
    let max_path = results.endpoints.iter().map(|e| e.path.len()).max().unwrap_or(0);
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
}
```

**Note:** Search drill commands appear regardless of `is_tty`. They are actionable data, not navigational hints — piped/scripted output benefits from them too.

### Tests to write

```
test_render_search_endpoint_includes_drill_command
  - SearchResults with one EndpointMatch { method: "GET", path: "/pets/{id}",
      summary: None, resource_slug: "pets", ... }
  - Assert output contains "phyllotaxis resources pets GET /pets/{id}"

test_render_search_endpoint_no_slug_omits_drill_command
  - EndpointMatch with resource_slug: ""
  - Assert "phyllotaxis resources" line is NOT in output

test_render_search_drill_command_shown_off_tty
  - Same endpoint, is_tty = false
  - Assert command still appears
```

**Independent of Tasks 1–4.**

---

## Task 6 — `PHYLLOTAXIS_SPEC` environment variable

**File:** `src/spec.rs`, function `resolve_spec_path`

**Precedence:** CLI `--spec` flag > `PHYLLOTAXIS_SPEC` env var > `.phyllotaxis.yaml` config file.

Insert the env var check between the flag block and the config-file block:

```rust
// Existing: --spec flag block
if let Some(spec) = spec_flag {
    // ... unchanged ...
}

// NEW: PHYLLOTAXIS_SPEC env var
if let Ok(env_spec) = std::env::var("PHYLLOTAXIS_SPEC") {
    if !env_spec.is_empty() {
        let path = PathBuf::from(&env_spec);
        let resolved = if path.is_absolute() { path } else { start_dir.join(path) };
        if resolved.is_file() {
            return Ok(resolved);
        }
        bail!("PHYLLOTAXIS_SPEC='{}' was set but the file was not found.", env_spec);
    }
}

// Existing: config file block (unchanged)
```

**Design decisions:**
- An empty `PHYLLOTAXIS_SPEC` is silently ignored (not an error), so `export PHYLLOTAXIS_SPEC=""` doesn't break anything.
- A non-empty value that can't be resolved is a hard error — not a silent fallthrough. Matches `--spec` behavior.
- No named-spec lookup via env var (YAGNI — the env var targets file-path use cases, not multi-spec configs).

`main.rs` does not change.

### Tests to write

```
test_resolve_uses_env_var_when_no_flag
  - Write a temp spec file
  - Set PHYLLOTAXIS_SPEC to its path
  - resolve_spec_path(None, ...) → Ok(spec_path)
  - Cleanup: remove_var

test_resolve_flag_wins_over_env_var
  - Set PHYLLOTAXIS_SPEC to file_a; pass file_b as spec_flag
  - Assert result is file_b

test_resolve_env_var_wins_over_config
  - Write config pointing to file_a; set PHYLLOTAXIS_SPEC to file_b
  - Assert result is file_b

test_resolve_env_var_not_found_is_error
  - Set PHYLLOTAXIS_SPEC to "/nonexistent/path.yaml"
  - Assert Err, message contains "PHYLLOTAXIS_SPEC"

test_resolve_env_var_empty_falls_through
  - Set PHYLLOTAXIS_SPEC to ""
  - Write config pointing to a valid file
  - Assert Ok(config_spec_path)
```

**Env var test caution:** `std::env::set_var` mutates global process state. Rust tests run in parallel threads. Use `std::env::remove_var` in a cleanup block after each test, and consider running env-var-related tests with `cargo test -- --test-threads=1` if flakiness occurs.

**Independent of Tasks 1–5.**

---

## Sequencing summary

| Order | Task | Depends on | Parallel with |
|-------|------|-----------|---------------|
| 1 | Add `drill_deeper` to `Endpoint`, `schema_ref` to `RequestBody` | — | 5, 6 |
| 2 | Populate `drill_deeper` in `get_endpoint_detail` | 1 | 5, 6 |
| 3 | Text renderer: emit endpoint drill-deeper | 1 | 4, 5, 6 |
| 4 | JSON renderer: assert `drill_deeper` in output | 1 | 3, 5, 6 |
| 5 | Search text renderer: emit full commands | — | 1, 2, 3, 4, 6 |
| 6 | `PHYLLOTAXIS_SPEC` env var | — | 1, 2, 3, 4, 5 |

Hard serial constraint: Task 1 before Tasks 2, 3, 4.

Wave 1 (parallel): Task 1 + Task 5 + Task 6
Wave 2 (parallel, after Task 1): Task 2 + Task 3 + Task 4

---

## Files changed

| File | Tasks |
|------|-------|
| `src/models/resource.rs` | 1 |
| `src/commands/resources.rs` | 2 |
| `src/render/text.rs` | 1 (literal updates), 3, 5 |
| `src/render/json.rs` | 1 (literal update), 4 |
| `src/spec.rs` | 6 |
| `src/main.rs` | none |
