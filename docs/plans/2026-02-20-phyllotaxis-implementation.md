# Phyllotaxis — Implementation Plan

**Date:** 2026-02-20
**Feature:** phyllotaxis — a Rust CLI for progressive disclosure of OpenAPI specs
**Design reference:** `docs/plans/2026-02-20-phyllotaxis-design.md`

---

## Overview

This plan organizes implementation into epics with bite-sized tasks. Each task follows a TDD rhythm where applicable: write a failing test, implement until it passes, commit. The plan assumes work is sequential within an epic but some epics can proceed in parallel once their dependencies are satisfied.

Dependency notation: "Requires Epic N, Task M" means that task must be committed before starting this one.

---

## Epic 1 — Project Scaffolding

Sets up the Rust project, dependencies, and a skeleton that compiles and exits cleanly.

### Task 1.1 — Initialize Cargo project

**File:** `/home/hhewett/.local/src/phyllotaxis/Cargo.toml`

Run `cargo init --name phyllotaxis` in the project root. Then edit `Cargo.toml` to set:

```toml
[package]
name = "phyllotaxis"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
openapiv3 = "2.2.0"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"

[dev-dependencies]
# (none yet — added in later epics)
```

Confirm `cargo check` passes.

**Dependencies:** None.

---

### Task 1.2 — Define CLI structure with clap

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

Replace the generated `main.rs` with a clap derive skeleton. Define the following structure:

- `Cli` struct with:
  - `spec: Option<PathBuf>` — `--spec <path>`, global
  - `json: bool` — `--json`, global
  - `expand: bool` — `--expand`, global
  - `command: Option<Commands>` — the subcommand (optional so bare `phyllotaxis` shows Level 0)

- `Commands` enum with variants:
  - `Resources { name: Option<String>, method: Option<String>, path: Option<String> }` — handles Levels 1, 2, and 3
  - `Schemas { name: Option<String> }` — handles schema listing (no name) and detail (with name)
  - `Auth` — no args
  - `Search { term: String }`
  - `Init` — no args

`main()` should parse args and `println!("OK")` for now — no command routing yet.

Confirm `cargo build` and `./target/debug/phyllotaxis --help` shows the expected subcommands.

**Dependencies:** Task 1.1.

---

### Task 1.3 — Set up module skeleton

**Files to create:**
- `/home/hhewett/.local/src/phyllotaxis/src/spec.rs` — empty module with `pub fn load() {}`
- `/home/hhewett/.local/src/phyllotaxis/src/models/mod.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/models/schema.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/mod.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/overview.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/schemas.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/auth.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/commands/init.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/render/mod.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs` — empty
- `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs` — empty

Declare all modules in `main.rs` with `mod spec; mod models; mod commands; mod render;` and the sub-modules in each `mod.rs`. Confirm `cargo check` passes with no dead_code warnings (use `#[allow(dead_code)]` at the top of each empty module for now).

**Dependencies:** Task 1.2.

---

### Task 1.4 — Add test fixture spec

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/fixtures/petstore.yaml`

Add a minimal OpenAPI 3.0 spec as a test fixture. Include enough coverage for all features:

```yaml
openapi: "3.0.4"
info:
  title: Petstore API
  version: "1.0.0"
  description: A simple petstore for testing phyllotaxis.
servers:
  - url: "https://{env}.example.com"
    variables:
      env:
        default: prod
        description: Environment name
tags:
  - name: Pets
    description: Pet management
  - name: Deprecated Pets
    description: "(Deprecated) Old pet endpoints"
  - name: "Experimental (Alpha)"
    description: Alpha feature endpoints
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
  schemas:
    Pet:
      type: object
      required: [id, name]
      properties:
        id:
          type: string
          format: uuid
          readOnly: true
          description: Unique identifier
        name:
          type: string
          description: Pet name
        status:
          type: string
          enum: [available, pending, sold]
        nickname:
          type: string
          nullable: true
          description: Optional nickname, can be cleared with null
        owner:
          $ref: '#/components/schemas/Owner'
    Owner:
      type: object
      properties:
        id:
          type: string
          readOnly: true
        name:
          type: string
    PetList:
      type: object
      allOf:
        - $ref: '#/components/schemas/Pet'
        - properties:
            tags:
              type: array
              items:
                type: string
    PetOrOwner:
      oneOf:
        - $ref: '#/components/schemas/Pet'
        - $ref: '#/components/schemas/Owner'
security:
  - bearerAuth: []
paths:
  /pets:
    get:
      tags: [Pets]
      summary: List all pets
      operationId: listPets
      parameters:
        - name: status
          in: query
          schema:
            type: string
            enum: [available, pending, sold]
          description: Filter by status
      responses:
        "200":
          description: A list of pets
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Pet'
    post:
      tags: [Pets]
      summary: Create a pet
      operationId: createPet
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Pet'
            example:
              name: Fido
              status: available
      responses:
        "201":
          description: Created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pet'
              example:
                id: "abc-123"
                name: Fido
                status: available
        "400":
          description: Invalid input
        "409":
          description: Pet with this name already exists
  /pets/{id}:
    get:
      tags: [Pets]
      summary: Get a pet by ID
      operationId: getPet
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
            format: uuid
          description: Pet identifier
      responses:
        "200":
          description: A pet
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pet'
        "404":
          description: Pet not found
    delete:
      tags: [Pets]
      summary: Delete a pet
      deprecated: true
      operationId: deletePet
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "204":
          description: Deleted
  /old-pets:
    get:
      tags: ["Deprecated Pets"]
      summary: Old pet listing
      deprecated: true
      responses:
        "200":
          description: Old list
  /pets/search:
    get:
      tags: ["Experimental (Alpha)"]
      summary: Fuzzy search pets
      responses:
        "200":
          description: Search results
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/PetOrOwner'
```

This fixture is used in all integration tests throughout the plan.

**Dependencies:** Task 1.1.

---

## Epic 2 — Spec Loading and Config

Implements the full spec resolution pipeline: config file discovery, `--spec` override, and `openapiv3` parsing.

### Task 2.1 — Define Config type

**File:** `/home/hhewett/.local/src/phyllotaxis/src/spec.rs`

Define:

```rust
#[derive(Debug, serde::Deserialize, Default)]
pub struct Config {
    pub spec: Option<String>,
    pub variables: Option<std::collections::HashMap<String, String>>,
}
```

Write a function `load_config(start_dir: &Path) -> Option<(Config, PathBuf)>` that:
1. Walks up from `start_dir` looking for `.phyllotaxis.yaml`
2. Stops at filesystem root
3. Returns `None` if not found, `Some((Config, config_dir))` if found and parsed (where `config_dir` is the directory containing the config file — needed to resolve relative spec paths)
4. On parse error, prints to stderr and returns `None`

Add a unit test `test_load_config_not_found` that calls the function with a temp dir containing no config file and asserts `None`.

Add a unit test `test_load_config_found` that writes a minimal `.phyllotaxis.yaml` to a temp dir and asserts the parsed spec path matches.

**Dependencies:** Task 1.3.

---

### Task 2.2 — Implement spec resolution

**File:** `/home/hhewett/.local/src/phyllotaxis/src/spec.rs`

Define a function `resolve_spec_path(spec_flag: Option<&str>, config: &Option<(Config, PathBuf)>, start_dir: &Path) -> Result<PathBuf, String>` that:
1. If `spec_flag` is `Some`, return that path (resolve relative to cwd)
2. Else if `config.spec` is `Some`, return that path (resolve relative to the config file's directory — use the `PathBuf` from `load_config`)
3. Else auto-detect: search `start_dir` and children (up to 2 levels) for files ending in `.yaml` or `.json` whose content contains `"openapi:"` (read first 200 bytes only, not full parse)
4. If none found, return `Err(...)` with a message explaining the resolution order

Add a unit test `test_resolve_prefers_flag` that confirms the `--spec` flag takes precedence.

Add a unit test `test_resolve_autodetect` that creates a fake spec in a temp dir and confirms it's found.

**Dependencies:** Task 2.1.

---

### Task 2.3 — Implement spec parsing

**File:** `/home/hhewett/.local/src/phyllotaxis/src/spec.rs`

Define a public struct:

```rust
pub struct LoadedSpec {
    pub api: openapiv3::OpenAPI,
    pub config: Config,
}
```

Define `pub fn load_spec(spec_flag: Option<&str>, start_dir: &Path) -> Result<LoadedSpec, String>` that:
1. Calls `load_config(start_dir)` to get `Option<(Config, PathBuf)>`; extract the `Config` for `LoadedSpec` and pass the tuple to `resolve_spec_path`
2. Calls `resolve_spec_path(...)` to get path
3. Reads the file to a string
4. Tries `serde_yaml::from_str` first; if it fails, tries `serde_json::from_str`
5. On failure, returns `Err(format!("Failed to parse {}: {}", path.display(), e))`
6. Returns `Ok(LoadedSpec { api, config })`

Add a unit test `test_parse_petstore` that loads `tests/fixtures/petstore.yaml` and asserts `api.info.title == "Petstore API"`.

Add a unit test `test_parse_bad_yaml` that passes garbage content and asserts an `Err` is returned.

**Dependencies:** Task 2.2.

---

### Task 2.4 — Wire spec loading into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In `main()`, after parsing CLI args, call `load_spec(cli.spec.as_deref(), &cwd)`. On error, print the error message to stderr and `std::process::exit(1)`. For now, print `api.info.title` to confirm it works.

Manually test: `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml` should print "Petstore API".

**Dependencies:** Task 2.3.

---

## Epic 3 — Core Models

Defines intermediate Rust types that commands populate and renderers consume. These types are the shared contract between command logic and rendering.

### Task 3.1 — Resource and endpoint models

**File:** `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs`

Define:

```rust
pub struct ResourceGroup {
    pub slug: String,          // CLI-friendly slug (lowercase, hyphens)
    pub display_name: String,  // Original tag name
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub is_alpha: bool,
    pub endpoints: Vec<Endpoint>,
}

pub struct Endpoint {
    pub method: String,        // "GET", "POST", etc.
    pub path: String,          // e.g. "/pets/{id}"
    pub summary: Option<String>,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub is_alpha: bool,
    pub external_docs: Option<ExternalDoc>,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: Vec<Response>,
    pub security_schemes: Vec<String>, // scheme names used
}

pub struct Parameter {
    pub name: String,
    pub location: ParameterLocation,  // Path, Query, Header
    pub required: bool,
    pub schema_type: String,   // e.g. "string", "integer"
    pub format: Option<String>,
    pub description: Option<String>,
    pub enum_values: Vec<String>,
}

pub enum ParameterLocation { Path, Query, Header }

pub struct RequestBody {
    pub content_type: String,
    pub fields: Vec<Field>,
    pub example: Option<serde_json::Value>,
}

pub struct Response {
    pub status_code: String,
    pub description: String,
    pub schema_ref: Option<String>,  // Schema name for "Returns: X"
    pub example: Option<serde_json::Value>,
}

pub struct ExternalDoc {
    pub url: String,
    pub description: Option<String>,
}
```

No logic — pure data structs. Add `#[derive(Debug, serde::Serialize)]` to all.

**Dependencies:** Task 1.3.

---

### Task 3.2 — Field model (shared by resources and schemas)

**File:** `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs`

Add to the same file:

```rust
pub struct Field {
    pub name: String,
    pub type_display: String,  // e.g. "string", "string/uuid", "Pet", "Pet[]"
    pub required: bool,
    pub optional: bool,        // explicitly optional (not in required, not required by context)
    pub nullable: bool,
    pub read_only: bool,
    pub description: Option<String>,
    pub enum_values: Vec<String>,
    pub default_value: Option<serde_json::Value>,
    pub example: Option<serde_json::Value>,  // per-field inline example from spec
    pub nested_schema_name: Option<String>,  // schema name for drill-deeper hint
    pub nested_fields: Vec<Field>,           // populated when --expand is used
}
```

**Dependencies:** Task 3.1.

---

### Task 3.3 — Schema model

**File:** `/home/hhewett/.local/src/phyllotaxis/src/models/schema.rs`

Define:

```rust
pub struct SchemaModel {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub external_docs: Option<super::resource::ExternalDoc>,
}

pub enum Composition {
    AllOf,   // fields are already merged/flattened into self.fields
    OneOf(Vec<String>),  // variant schema names
    AnyOf(Vec<String>),  // variant schema names
}
```

**Dependencies:** Task 3.2.

---

### Task 3.4 — Slugification utility

**File:** `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs`

Add a function `pub fn slugify(tag_name: &str) -> String` that:
1. Strips trailing ` (Deprecated)`, ` (deprecated)`, ` (Alpha)`, ` (alpha)` (case-insensitive)
2. Splits PascalCase words by inserting hyphens before uppercase letters that follow lowercase letters (`DiscoveryIntegration` → `discovery-integration`)
3. Replaces spaces with hyphens
4. Lowercases the result

Add unit tests:
- `test_slugify_spaces`: `"Access Policies"` → `"access-policies"`
- `test_slugify_pascal`: `"DiscoveryIntegration"` → `"discovery-integration"`
- `test_slugify_deprecated_stripped`: `"Old Pets (Deprecated)"` → `"old-pets"`
- `test_slugify_alpha_stripped`: `"New Feature (Alpha)"` → `"new-feature"`

**Dependencies:** Task 3.1.

---

### Task 3.5 — Status detection utility

**File:** `/home/hhewett/.local/src/phyllotaxis/src/models/resource.rs`

Add two functions:
- `pub fn is_deprecated_tag(tag_name: &str) -> bool` — returns true if the tag name contains `(Deprecated)` or `(deprecated)`, or if the tag's extensions contain `x-deprecated: true`
- `pub fn is_alpha_tag(tag_name: &str) -> bool` — returns true if tag name contains `(Alpha)` or `(alpha)`, or extensions contain `x-alpha: true`

Also add:
- `pub fn detect_status_from_extensions(extensions: &indexmap::IndexMap<String, serde_json::Value>) -> (bool, bool)` — returns `(is_deprecated, is_alpha)` by checking the `x-deprecated` and `x-alpha` keys

Add unit tests:
- `test_deprecated_by_name`: `"Old Pets (Deprecated)"` → true
- `test_alpha_by_name`: `"Beta Feature (Alpha)"` → true
- `test_not_deprecated`: `"Access Policies"` → false

**Dependencies:** Task 3.1.

---

### Task 3.6 — Ref resolution utility

**File:** `/home/hhewett/.local/src/phyllotaxis/src/spec.rs`

Add a function `pub fn resolve_schema<'a>(spec: &'a openapiv3::OpenAPI, ref_or: &'a openapiv3::ReferenceOr<openapiv3::Schema>) -> Option<(&'a openapiv3::Schema, Option<&'a str>)>` that:
1. If `Item(schema)`, returns `(schema, None)`
2. If `Reference { reference }`, parses the `#/components/schemas/Name` string, looks it up in `spec.components.schemas`, and returns `(schema, Some("Name"))`
3. Returns `None` if the ref cannot be resolved (log to stderr)

Also add `pub fn schema_name_from_ref(reference: &str) -> Option<&str>` that extracts `"Name"` from `"#/components/schemas/Name"`.

Add unit tests:
- `test_schema_name_from_ref`: `"#/components/schemas/Pet"` → `Some("Pet")`
- `test_schema_name_invalid`: `"#/components/other/Pet"` → `None`

**Dependencies:** Task 2.3.

---

## Epic 4 — Level 0: Overview Command

### Task 4.1 — Build overview model

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/overview.rs`

Define a private struct:

```rust
struct OverviewData {
    title: String,
    description: Option<String>,
    base_urls: Vec<String>,       // resolved if config.variables is set, template otherwise
    server_variables: Vec<ServerVar>,
    auth_schemes: Vec<String>,    // display name of each security scheme
    resource_count: usize,
    schema_count: usize,
}

struct ServerVar {
    name: String,
    required: bool,
    description: Option<String>,
    default: Option<String>,
}
```

Add a function `pub fn build(loaded: &LoadedSpec) -> OverviewData` that:
1. Gets `api.info.title` and truncates `api.info.description` to 200 chars
2. For each server in `api.servers`, resolve URL variables using `config.variables` if present (do a string replacement of `{var}` with the config value), otherwise keep the template URL. Collect `ServerVar`s from `server.variables` entries.
3. Collect security scheme display names from `api.components.security_schemes` keys
4. Count unique tags (resource_count) and schema count from `api.components.schemas`

**Dependencies:** Task 2.3, Task 3.1.

---

### Task 4.2 — Render overview as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add a function `pub fn render_overview(data: &OverviewData) -> String` that produces output matching the design:

```
API: {title}
{description (if present)}
Base URL: {url}
  Variables:
    {name}  ({required/optional})  {description}
Auth: {scheme display}

Commands:
  phyllotaxis resources    List all resource groups ({N} available)
  phyllotaxis schemas      List all data models ({N} available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

If multiple servers exist, print "Base URLs:" with each one listed. If no description, skip that line. If no auth schemes, skip the Auth line.

Add a unit test `test_render_overview_basic` that constructs a minimal `OverviewData` and asserts the output contains "API: Petstore API" and "phyllotaxis resources".

**Dependencies:** Task 4.1.

---

### Task 4.3 — Render overview as JSON

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add a function `pub fn render_overview(data: &OverviewData) -> String` that serializes to JSON. The JSON shape should mirror the text output structure:

```json
{
  "title": "...",
  "description": "...",
  "servers": [{ "url": "...", "variables": [...] }],
  "auth": ["bearer"],
  "resource_count": 31,
  "schema_count": 143,
  "commands": { "resources": "phyllotaxis resources", ... }
}
```

Make `OverviewData` (and nested types) `#[derive(serde::Serialize)]`. Use `serde_json::to_string_pretty`.

**Dependencies:** Task 4.2.

---

### Task 4.4 — Wire overview command into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In `main()`, when `cli.command` is `None` (bare `phyllotaxis`):
1. Build overview data
2. If `--json`, call `render::json::render_overview`; else call `render::text::render_overview`
3. Print the result

Manually test: `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml` should print the overview.

**Dependencies:** Task 4.3.

---

## Epic 5 — Level 1: Resource Listing

### Task 5.1 — Extract resource groups from spec

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

Add a function `pub fn extract_resource_groups(api: &openapiv3::OpenAPI) -> Vec<ResourceGroup>` that:
1. Iterates `api.tags` (the global tags list) to get display names and descriptions
2. For each tag, calls `slugify()` and `is_deprecated_tag()` / `is_alpha_tag()`
3. Collects all operations (`api.paths` → each path item → each HTTP method) and groups them by tag. Each operation can have multiple tags — assign it to all matching groups.
4. Populates each `ResourceGroup.endpoints` with `Endpoint` structs containing only `method`, `path`, `summary`, `is_deprecated` (from operation's `deprecated` field), and `is_alpha` (from operation's `x-alpha` extension)
5. Returns groups sorted alphabetically by slug

Important: if a tag has no operations, still include it in the list (shows up in resources list). If an operation has a tag that's not in `api.tags`, create an implicit group for it.

Add a unit test `test_extract_petstore_groups` that loads the petstore fixture and asserts three groups are returned: `"pets"`, `"deprecated-pets"`, and `"experimental"` (alpha).

**Dependencies:** Task 3.4, Task 3.5, Task 2.3.

---

### Task 5.2 — Render resource listing as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add a function `pub fn render_resource_list(groups: &[ResourceGroup]) -> String` that produces:

```
Resources:
  access-policies    [DEPRECATED]  Access policies
  pets                             Pet management
  ...

Drill deeper:
  phyllotaxis resources <name>
```

Rules:
- Column-align the slug, marker, and description using the longest slug length
- `[DEPRECATED]` marker after slug for deprecated groups; `[ALPHA]` for alpha groups
- Description is the tag's description if present; blank otherwise
- No description placeholder if missing

Add a unit test `test_render_resource_list` that builds two groups (one deprecated) and checks the output contains `[DEPRECATED]` for the right one.

**Dependencies:** Task 5.1.

---

### Task 5.3 — Render resource listing as JSON

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_resource_list(groups: &[ResourceGroup]) -> String` that produces:

```json
{
  "resources": [
    {
      "slug": "pets",
      "display_name": "Pets",
      "description": "Pet management",
      "deprecated": false,
      "alpha": false,
      "endpoint_count": 4
    }
  ],
  "drill_deeper": "phyllotaxis resources <name>"
}
```

**Dependencies:** Task 5.2.

---

### Task 5.4 — Wire Level 1 into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In the `Commands::Resources { name: None, .. }` branch:
1. Call `extract_resource_groups`
2. Render and print

Manually test: `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml resources` should show "pets" and "deprecated-pets".

**Dependencies:** Task 5.3.

---

## Epic 6 — Level 2: Resource Detail

### Task 6.1 — Look up a resource group by slug

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

Add `pub fn find_resource_group(groups: &[ResourceGroup], slug: &str) -> Option<usize>` that returns the index of the group whose `slug` matches (case-insensitive).

Add a separate function `pub fn suggest_similar(groups: &[ResourceGroup], slug: &str) -> Vec<&str>` that returns up to 3 group slugs that contain `slug` as a substring (case-insensitive). Used in "not found" error messages.

Add a unit test `test_find_group_exact` and `test_find_group_not_found`.

**Dependencies:** Task 5.1.

---

### Task 6.2 — Populate full endpoint detail for Level 2

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

The Level 2 view shows all endpoints for a group with method, path, and summary. The Level 1 extraction already captures this data. Verify that `ResourceGroup.endpoints` is fully populated from Task 5.1.

Add a function `pub fn get_resource_detail(api: &openapiv3::OpenAPI, group: &ResourceGroup) -> ResourceGroup` that re-extracts the group with full endpoint data. For now this is the same as Level 1 data since both levels share the same struct — no additional work needed. Just ensure `summary` is populated.

**Dependencies:** Task 6.1.

---

### Task 6.3 — Render resource detail as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_resource_detail(group: &ResourceGroup) -> String` that produces:

```
Resource: {display_name}
Description: {description (if present, omit entire line if absent)}

Endpoints:
  GET    /pets           List all pets
  POST   /pets           Create a pet
  GET    /pets/{id}      Get a pet by ID
  DELETE /pets/{id}      [DEPRECATED] Delete a pet

Drill deeper:
  phyllotaxis resources pets GET /pets
  phyllotaxis resources pets POST /pets
  ...
```

Rules:
- Column-align METHOD (widest is DELETE, 6 chars) and path
- `[DEPRECATED]` after summary for deprecated endpoints; `[ALPHA]` for alpha
- Drill-deeper hints for every endpoint in the group

Add a unit test `test_render_resource_detail` using a constructed `ResourceGroup`.

**Dependencies:** Task 6.2.

---

### Task 6.4 — Render resource detail as JSON

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_resource_detail(group: &ResourceGroup) -> String` producing:

```json
{
  "slug": "pets",
  "display_name": "Pets",
  "description": "...",
  "deprecated": false,
  "alpha": false,
  "endpoints": [
    {
      "method": "GET",
      "path": "/pets",
      "summary": "List all pets",
      "deprecated": false,
      "alpha": false
    }
  ],
  "drill_deeper": ["phyllotaxis resources pets GET /pets", ...]
}
```

**Dependencies:** Task 6.3.

---

### Task 6.5 — Wire Level 2 into main and handle not-found

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In the `Commands::Resources { name: Some(name), method: None, .. }` branch:
1. Extract groups, look up by slug
2. If not found: print `"Resource '{name}' not found."` and suggestions from `suggest_similar`, then exit 1
3. Render and print

Manually test: `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml resources pets` shows the resource detail.

**Dependencies:** Task 6.4.

---

## Epic 7 — Level 3: Endpoint Detail

### Task 7.1 — Build full endpoint detail model

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

Add `pub fn get_endpoint_detail(api: &openapiv3::OpenAPI, group: &ResourceGroup, method: &str, path: &str) -> Option<Endpoint>` that:
1. Finds the operation matching method + path
2. Merges path-level parameters with operation-level parameters (operation takes precedence on name collision, per OpenAPI spec)
3. Populates `Parameter` structs: resolves `$ref` parameters from `components/parameters`
4. Populates `RequestBody`: finds the `application/json` content type, resolves the schema, and builds a flat `Vec<Field>` from its properties. Marks fields as required if they appear in the schema's `required` array. Extracts the `example` value from the media type object.
5. Populates `Response` for each response code: extracts `description`, finds the first response schema ref name (for "Returns: X"), and extracts the first `application/json` example.
6. Populates `security_schemes` from the operation's security requirement, falling back to the global security requirement.

This is the most complex function. Keep it focused: `$ref` schema resolution uses `resolve_schema` from `spec.rs`. Request body field building delegates to `build_fields` from Task 7.2.

**Dependencies:** Task 3.2, Task 3.6, Task 6.2, Task 7.2.

---

### Task 7.2 — Build fields from schema properties

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

Extract a helper function `fn build_fields(api: &openapiv3::OpenAPI, schema: &openapiv3::Schema, required_fields: &[String]) -> Vec<Field>` that:
1. Iterates schema properties (from `schema.schema_kind` if it's `Type::Object`)
2. For each property, calls `resolve_schema` to follow any `$ref`
3. Determines `type_display`:
   - Primitive: `"string"`, `"integer"`, `"boolean"`, `"number"`
   - With format: `"string/uuid"`, `"string/date-time"`
   - Object ref: `"SchemaName"` (use the ref name)
   - Array of ref: `"SchemaName[]"`
   - Array of primitive: `"string[]"`
4. Sets `required`, `optional`, `nullable`, `read_only` from schema properties
5. Sets `enum_values` from the schema's `enum` field if present
6. Sets `example` from the property's `example` field if present (for inline display of short per-field examples like datetime formats)
7. Sets `nested_schema_name` to the ref name (for drill-deeper hints)
7. Does NOT recurse — leaves `nested_fields` empty (expansion is handled separately in Epic 8)

Add a unit test `test_build_fields_pet` that loads the petstore fixture, resolves the `Pet` schema, and asserts fields include `id` (type `string/uuid`, read_only), `name` (required), `status` (with enum values), and `nickname` (nullable).

**Dependencies:** Task 3.6.

---

### Task 7.3 — Handle allOf flattening in field building

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/resources.rs`

Extend `build_fields` to handle `allOf` schemas. When a schema's `schema_kind` is `allOf`:
1. For each referenced schema in the `allOf` list, resolve it and recursively call `build_fields`
2. Merge all resulting fields into a single flat list (deduplication by field name: later entries win)
3. The merged `required` list is the union of all constituent schemas' required arrays

Add a unit test `test_build_fields_allof` that loads the `PetList` schema from the petstore fixture and asserts it contains all fields from `Pet` plus `tags`.

**Dependencies:** Task 7.2.

---

### Task 7.4 — Render endpoint detail as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_endpoint_detail(endpoint: &Endpoint, path: &str) -> String` that produces the Level 3 output from the design document:

```
{METHOD} {path}
{description (if present, truncated at 500 chars)}

Authentication: {scheme} (required)

Path Parameters:
  {name}    {type}    (required)   {description}
  -- or "(none)" if empty --

Query Parameters:
  {name}    {type}    {description}
  -- or "(none)" if empty --

Request Body (application/json):
  {field rows}
  -- or "(none)" if no request body --

Request Example:
  {formatted JSON}
  -- only if example present --

Response: {status_code} {description}
  Returns: {SchemaName}
  -- only if schema_ref present --

Response Example:
  {formatted JSON}
  -- only if example present --

Errors:
  {code}  {description}
  -- only non-2xx responses --

Drill deeper:
  phyllotaxis schemas {SchemaName}
  -- for each unique nested_schema_name in fields --
```

Field row format: `  {name}    {type_display}    ({modifiers})   {description}`
Where modifiers: "required", "optional", "optional, nullable", "read-only", etc. — combine as needed.
Enum values inline after description: `Enum: [active, inactive, sold]`.
Default values: `Default: true`.
Column-align name and type_display columns.

Add a unit test `test_render_endpoint_detail_post_pets` that builds a sample `Endpoint` and asserts the output contains "Request Body", "Authentication", and "Errors".

**Dependencies:** Task 7.1.

---

### Task 7.5 — Render endpoint detail as JSON

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_endpoint_detail(endpoint: &Endpoint, path: &str) -> String` producing:

```json
{
  "method": "POST",
  "path": "/pets",
  "description": "...",
  "authentication": ["bearer"],
  "path_parameters": [],
  "query_parameters": [],
  "request_body": {
    "content_type": "application/json",
    "fields": [
      {
        "name": "name",
        "type": "string",
        "required": true,
        "optional": false,
        "nullable": false,
        "read_only": false,
        "description": "Pet name",
        "enum_values": [],
        "default": null
      }
    ],
    "example": { ... }
  },
  "responses": [...],
  "drill_deeper": ["phyllotaxis schemas Pet"]
}
```

**Dependencies:** Task 7.4.

---

### Task 7.6 — Wire Level 3 into main and handle not-found

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In the `Commands::Resources { name: Some(name), method: Some(method), path: Some(path) }` branch:
1. Extract groups, find group by slug
2. Call `get_endpoint_detail`
3. If not found: print `"Endpoint '{method} {path}' not found in resource '{name}'."` and exit 1
4. Render and print

Manually test:
- `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml resources pets POST /pets`
- `./target/debug/phyllotaxis --spec tests/fixtures/petstore.yaml resources pets GET /pets/{id}`

**Dependencies:** Task 7.5.

---

## Epic 8 — Schemas Command

### Task 8.1 — Build schema listing

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/schemas.rs`

Add `pub fn list_schemas(api: &openapiv3::OpenAPI) -> Vec<String>` that returns all schema names from `api.components.schemas` sorted alphabetically.

Add `pub fn find_schema(api: &openapiv3::OpenAPI, name: &str) -> Option<openapiv3::Schema>` that looks up a schema by exact name (case-sensitive first, then case-insensitive fallback). Resolves `ReferenceOr` to the concrete schema.

Add `pub fn suggest_similar_schemas(api: &openapiv3::OpenAPI, name: &str) -> Vec<String>` returning up to 3 schema names containing `name` as a substring (case-insensitive).

Unit test `test_list_petstore_schemas` asserts "Pet", "Owner", "PetList", "PetOrOwner" are in the list.

**Dependencies:** Task 3.3, Task 3.6.

---

### Task 8.2 — Build schema model without expansion

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/schemas.rs`

Add `pub fn build_schema_model(api: &openapiv3::OpenAPI, name: &str, schema: &openapiv3::Schema) -> SchemaModel` that:
1. Sets `name` and truncates `description` to 500 chars
2. Detects composition:
   - If schema has `allOf`: set `composition = Some(Composition::AllOf)` and call `build_fields` with allOf flattening
   - If schema has `oneOf`: set `composition = Some(Composition::OneOf(variant_names))` and set `fields = []`
   - If schema has `anyOf`: set `composition = Some(Composition::AnyOf(variant_names))` and set `fields = []`
   - Otherwise: call `build_fields` for properties
3. Sets `external_docs` from schema's `external_docs` field

Unit test `test_build_pet_schema` asserts 5 fields, no composition, `external_docs = None`.
Unit test `test_build_petlist_schema` asserts `composition = Some(Composition::AllOf)` and fields include those from both `Pet` and `PetList`.

**Dependencies:** Task 7.3, Task 8.1.

---

### Task 8.3 — Implement schema expansion with depth limit and cycle detection

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/schemas.rs`

Add `pub fn expand_field(api: &openapiv3::OpenAPI, field: &mut Field, depth: usize, visited: &mut Vec<String>)` that:
1. If `depth == 0`, sets `field.nested_fields = []` and changes `field.type_display` to `{SchemaName} [max depth]` if a schema name is present — leave it unexpanded
2. If `field.nested_schema_name` is in `visited`, sets `field.type_display` to `[circular: {SchemaName}]` and returns
3. Otherwise: resolves the schema by name, calls `build_fields`, sets `field.nested_fields`; pushes the schema name to `visited`, recurses with `depth - 1` on each nested field, then pops the schema name from `visited`

Add `pub fn expand_schema(api: &openapiv3::OpenAPI, model: &mut SchemaModel)` that calls `expand_field` with `depth = 5` and an empty `visited` vec on each field.

Unit test `test_expand_pet_schema`: load the `Pet` schema, expand it, assert `owner.nested_fields` contains `id` and `name` fields from `Owner`.
Unit test `test_cycle_detection`: build a schema that references itself (manually constructed, not from fixture) and assert the circular field is marked rather than panicking.

**Dependencies:** Task 8.2.

---

### Task 8.4 — Render schema listing as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_schema_list(names: &[String]) -> String` that produces:

```
Schemas ({N} total):
  AccessPolicy
  AccessPolicyV2DTO
  ...

Drill deeper:
  phyllotaxis schemas <name>
```

**Dependencies:** Task 8.1.

---

### Task 8.5 — Render schema detail as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_schema_detail(model: &SchemaModel, expanded: bool) -> String` that produces:

```
Schema: {name}{" (expanded)" if expanded}
{description if present}

{composition block if present}
  allOf: fields are merged below
  -- OR --
  One of:
    phyllotaxis schemas Variant1
    phyllotaxis schemas Variant2
  -- OR --
  Any of:
    phyllotaxis schemas Variant1

Fields:
  {field rows, same format as Level 3 request body}
  {nested_fields indented with 2 extra spaces if expanded}

Related schemas:
  phyllotaxis schemas {nested_schema_name}
  -- deduplicated, only when NOT expanded --

{externalDocs block if present}
See also: {url}
  {description if present}
```

For expanded fields, render nested fields indented under the parent:
```
  owner      Owner:
    id         string    (read-only)
    name       string
```

Add a unit test `test_render_schema_detail_simple` and `test_render_schema_detail_expanded`.

**Dependencies:** Task 8.3.

---

### Task 8.6 — Render schemas as JSON

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_schema_list(names: &[String]) -> String` and `pub fn render_schema_detail(model: &SchemaModel) -> String`.

The schema detail JSON should serialize the full `SchemaModel` including nested fields. The `composition` field should serialize as an object with a `type` key (`"allOf"`, `"oneOf"`, `"anyOf"`) and for `oneOf`/`anyOf`, a `variants` array.

**Dependencies:** Task 8.5.

---

### Task 8.7 — Wire schemas command into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

In the `Commands::Schemas` branch:
- `name: None` → list schemas
- `name: Some(name)` → look up schema, build model; if `--expand`, call `expand_schema`; render
- If not found: print error with suggestions from `suggest_similar_schemas`, exit 1

**Dependencies:** Task 8.6.

---

## Epic 9 — Auth Command

### Task 9.1 — Build auth model

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/auth.rs`

Define:

```rust
pub struct AuthModel {
    pub schemes: Vec<SecurityScheme>,
}

pub struct SecurityScheme {
    pub name: String,             // key from components/securitySchemes
    pub scheme_type: String,      // "http", "apiKey", "oauth2", "openIdConnect"
    pub detail: String,           // e.g. "bearer" for http type, location+name for apiKey
    pub description: Option<String>,
    pub usage_count: usize,       // number of operations using this scheme
}
```

Add `pub fn build_auth_model(api: &openapiv3::OpenAPI) -> AuthModel` that:
1. Iterates `api.components.security_schemes`
2. For each scheme, determines `scheme_type` and `detail` from the `SecurityScheme` enum variants in `openapiv3`
3. Counts operations that reference this scheme by name: iterate all operations and check their `security` list, plus check the global `api.security` list

Unit test `test_build_auth_petstore` asserts one scheme named "bearerAuth" with scheme_type "http" and detail "bearer".

**Dependencies:** Task 2.3.

---

### Task 9.2 — Render auth as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_auth(model: &AuthModel) -> String` that produces the design's auth output:

```
Authentication:
  bearer (HTTP)
    Scheme: bearer
    Description: JWT token for API access

  Used by: 4 operations (all endpoints)

Drill deeper:
  phyllotaxis resources    Browse endpoints by resource group
```

If `usage_count` equals the total operation count, add "(all endpoints)" qualifier.

**Dependencies:** Task 9.1.

---

### Task 9.3 — Render auth as JSON and wire into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_auth(model: &AuthModel) -> String` with the full auth model serialized.

In `main.rs`, wire `Commands::Auth` to build and render.

**Dependencies:** Task 9.2.

---

## Epic 10 — Search Command

### Task 10.1 — Implement search across all types

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/search.rs`

Define:

```rust
pub struct SearchResults {
    pub term: String,
    pub resources: Vec<ResourceMatch>,
    pub endpoints: Vec<EndpointMatch>,
    pub schemas: Vec<SchemaMatch>,
}

pub struct ResourceMatch {
    pub slug: String,
    pub description: Option<String>,
}

pub struct EndpointMatch {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub resource_slug: String,
}

pub struct SchemaMatch {
    pub name: String,
}
```

Add `pub fn search(api: &openapiv3::OpenAPI, term: &str) -> SearchResults` that:
1. Lowercases `term` for comparison
2. Searches resource group slugs and descriptions (case-insensitive substring)
3. Searches endpoint paths, summaries, and descriptions
4. Searches schema names
5. Returns results grouped in fixed order: resources, endpoints, schemas
6. No deduplication needed — each match type is independent

Unit test `test_search_workload` (or with "pet" against petstore fixture): asserts resources, endpoints, and schemas containing the term are all returned.

**Dependencies:** Task 5.1, Task 8.1.

---

### Task 10.2 — Render search results as plain text

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/text.rs`

Add `pub fn render_search(results: &SearchResults) -> String` that produces the design's search output:

```
Results for "{term}":

Resources:
  pets    Pet management
  ...

Endpoints:
  GET  /pets           List all pets
  POST /pets           Create a pet
  ...

Schemas:
  Pet
  PetList
  ...

Drill deeper:
  phyllotaxis resources pets
  phyllotaxis schemas Pet
```

If a section has zero results, omit that section header entirely.

**Dependencies:** Task 10.1.

---

### Task 10.3 — Render search as JSON and wire into main

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Add `pub fn render_search(results: &SearchResults) -> String` serializing the full `SearchResults`.

In `main.rs`, wire `Commands::Search { term }` to call `search()` and render.

**Dependencies:** Task 10.2.

---

## Epic 11 — Init Command

### Task 11.1 — Implement framework detection

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/init.rs`

Define a detection table as a static slice:

```rust
struct Framework { name: &'static str, signatures: &'static [&'static str] }

static FRAMEWORKS: &[Framework] = &[
    Framework { name: "Astro", signatures: &["astro.config.mjs", "astro.config.ts"] },
    Framework { name: "Docusaurus", signatures: &["docusaurus.config.js", "docusaurus.config.ts"] },
    Framework { name: "Hugo", signatures: &["hugo.toml", "hugo.yaml", "config.toml"] },
    Framework { name: "Jekyll", signatures: &["_config.yml", "_config.yaml"] },
    Framework { name: "MkDocs", signatures: &["mkdocs.yml", "mkdocs.yaml"] },
];
```

Add `pub fn detect_framework(dir: &Path) -> Option<&'static str>` that returns the first matching framework name by checking if any signature file exists in `dir`.

Add `pub fn find_spec_candidates(dir: &Path, framework: Option<&str>) -> Vec<PathBuf>` that:
- Searches `dir` and two levels of children for `*.yaml`, `*.yml`, `*.json` files
- Filters to those that contain `"openapi:"` in the first 200 bytes
- Common spec locations by framework (Astro: `src/content/`, Docusaurus: `static/`, Hugo: `static/`, Jekyll: `assets/`, MkDocs: `docs/`) — search those first, then broader search

**Dependencies:** Task 1.3.

---

### Task 11.2 — Implement interactive init flow

**File:** `/home/hhewett/.local/src/phyllotaxis/src/commands/init.rs`

Add `pub fn run_init(start_dir: &Path)` that:
1. Checks if `.phyllotaxis.yaml` already exists in `start_dir`. If so, prints "Already initialized. Edit .phyllotaxis.yaml to update." and returns.
2. Calls `detect_framework` and prints the detected framework (or "No doc framework detected.")
3. Calls `find_spec_candidates` and lists up to 5 candidates
4. Prompts the user: "Select a spec file (enter number) or type a path manually: " using `std::io::stdin().read_line()`
5. Validates the selected path exists
6. Writes `.phyllotaxis.yaml` to `start_dir`:

```yaml
spec: {relative_path_from_start_dir}
```

7. Prints "Initialized. Run `phyllotaxis` to see your API overview."

The interactive prompts use `eprint!` / `eprintln!` for prompts and `stdin().read_line()` for input, so stdout stays clean for piping.

Wire `Commands::Init` in `main.rs` to call `run_init(&cwd)`. This command does not use `--spec` or `--json`.

**Dependencies:** Task 11.1.

---

## Epic 12 — JSON Output Consistency

### Task 12.1 — Audit all JSON renderers for consistency

**File:** `/home/hhewett/.local/src/phyllotaxis/src/render/json.rs`

Review all `render_*` functions in `json.rs` and verify:
1. Every function uses `serde_json::to_string_pretty` (no manual JSON construction)
2. Every function returns valid, parseable JSON (write a quick test that calls `serde_json::from_str` on each output)
3. The `drill_deeper` field is always present as an array of command strings (even if empty)
4. Boolean fields use `false` not `null` for absent markers
5. Optional string fields use `null` not empty string

Add a test `test_all_json_outputs_parse` that constructs minimal data for each renderer and asserts `serde_json::from_str::<serde_json::Value>(output).is_ok()`.

**Dependencies:** All render/json tasks (Epics 4–11).

---

### Task 12.2 — Verify --json flag propagation

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

Audit every command branch in `main()` and confirm `cli.json` is checked before rendering. Every command that renders should have exactly one branch point:

```rust
let output = if cli.json {
    render::json::render_*(...)
} else {
    render::text::render_*(...)
};
println!("{}", output);
```

No command should render without checking this flag. Fix any that don't.

**Dependencies:** Task 12.1.

---

## Epic 13 — Error Handling

### Task 13.1 — Consistent error formatting

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

Define a helper at the top of `main.rs`:

```rust
fn die(msg: &str) -> ! {
    eprintln!("Error: {}", msg);
    std::process::exit(1);
}
```

Replace all `eprintln!` + `process::exit(1)` patterns throughout `main()` with `die(...)`. This is a single-function utility, not an abstraction layer — just removes repetition.

**Dependencies:** Task 1.2.

---

### Task 13.2 — Not-found errors with suggestions

**File:** `/home/hhewett/.local/src/phyllotaxis/src/main.rs`

Verify all "not found" paths print suggestions. The pattern for each:
- Resource not found: print `"Resource '{name}' not found."` + up to 3 similar slugs from `suggest_similar()`
- Endpoint not found: print `"Endpoint '{METHOD} {path}' not found in '{resource}'."` (no suggestions needed — path must be exact)
- Schema not found: print `"Schema '{name}' not found."` + up to 3 similar names from `suggest_similar_schemas()`

Each error prints to stderr (use `eprintln!`), not stdout.

**Dependencies:** Task 6.1, Task 8.1, Task 13.1.

---

## Epic 14 — Integration Tests

### Task 14.1 — Integration test: overview command

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

Use `std::process::Command` to run the compiled binary against the petstore fixture. Add a helper:

```rust
fn run(args: &[&str]) -> (String, String, i32) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .args(args)
        .output()
        .expect("failed to run binary");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}
```

Add `test_overview_text`: runs `phyllotaxis --spec tests/fixtures/petstore.yaml`, asserts exit code 0, stdout contains "API: Petstore API", "phyllotaxis resources", "phyllotaxis schemas".

Add `test_overview_json`: runs same with `--json`, asserts output parses as JSON and `json["title"] == "Petstore API"`.

**Dependencies:** Task 4.4.

---

### Task 14.2 — Integration test: resources commands

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

Add:
- `test_resources_list`: asserts "pets", "deprecated-pets", and "experimental" in output; `[DEPRECATED]` marker present for deprecated group; `[ALPHA]` marker present for alpha group
- `test_resources_detail`: `resources pets` shows all 4 endpoints with methods
- `test_resources_endpoint`: `resources pets GET /pets` shows "Query Parameters", "Response: 200"
- `test_resources_endpoint_post`: `resources pets POST /pets` shows "Request Body", field rows, "Request Example"
- `test_resources_not_found`: `resources notexist` exits with code 1 and stderr contains "not found"

**Dependencies:** Task 7.6.

---

### Task 14.3 — Integration test: schemas commands

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

Add:
- `test_schemas_list`: asserts "Pet", "Owner", "PetList" in output
- `test_schema_detail_pet`: `schemas Pet` shows all 5 fields, `string/uuid` for id, `Enum:` for status
- `test_schema_detail_expanded`: `schemas Pet --expand` shows `owner.id` and `owner.name` inline
- `test_schema_allof`: `schemas PetList` shows fields from both Pet and PetList (allOf merged)
- `test_schema_oneof`: `schemas PetOrOwner` shows "One of:" with links to Pet and Owner variant schemas
- `test_schema_not_found`: `schemas NotReal` exits 1 and suggests similar

**Dependencies:** Task 8.7.

---

### Task 14.4 — Integration test: auth and search commands

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

Add:
- `test_auth`: `auth` shows "bearerAuth", "HTTP", "bearer"
- `test_search_pet`: `search pet` returns Resources section with "pets", Endpoints section with GET /pets entries, Schemas section with "Pet", "PetList"
- `test_search_no_results`: `search xyzzy123` shows no results (empty sections omitted) and exits 0

**Dependencies:** Task 9.3, Task 10.3.

---

### Task 14.5 — Integration test: global flag and error cases

**File:** `/home/hhewett/.local/src/phyllotaxis/tests/integration_tests.rs`

Add:
- `test_spec_not_found`: run without `--spec` and no `.phyllotaxis.yaml` in a temp dir, assert exit code 1 and stderr contains "not found"
- `test_invalid_spec`: `--spec /dev/null`, assert exit code 1 and stderr contains "Failed to parse"
- `test_json_flag_all_commands`: run overview, resources, schemas, auth, search all with `--json`, assert each parses as valid JSON

**Dependencies:** Task 12.2.

---

## Dependency Summary

This table shows the minimum unblocked starting tasks for parallel work:

| Epic | Can start after |
|------|----------------|
| Epic 1 (Scaffolding) | Nothing |
| Epic 2 (Spec loading) | Epic 1 |
| Epic 3 (Models) | Epic 1 |
| Epic 4 (Overview) | Epics 2, 3 |
| Epic 5 (L1 Resources) | Epics 2, 3 |
| Epic 6 (L2 Resource detail) | Epic 5 |
| Epic 7 (L3 Endpoint detail) | Epic 6 |
| Epic 8 (Schemas) | Epics 2, 3, 7.3 |
| Epic 9 (Auth) | Epic 2 |
| Epic 10 (Search) | Epics 5, 8 |
| Epic 11 (Init) | Epic 1 |
| Epic 12 (JSON consistency) | All render tasks |
| Epic 13 (Error handling) | Epic 1 |
| Epic 14 (Integration tests) | All command tasks |

## File Index

All files that will exist when implementation is complete:

```
/home/hhewett/.local/src/phyllotaxis/
  Cargo.toml
  src/
    main.rs
    spec.rs
    models/
      mod.rs
      resource.rs          (ResourceGroup, Endpoint, Parameter, Field, slugify, is_deprecated_tag, etc.)
      schema.rs            (SchemaModel, Composition)
    commands/
      mod.rs
      overview.rs          (build)
      resources.rs         (extract_resource_groups, find_resource_group, get_endpoint_detail, build_fields)
      schemas.rs           (list_schemas, find_schema, build_schema_model, expand_schema)
      auth.rs              (build_auth_model)
      search.rs            (search)
      init.rs              (detect_framework, find_spec_candidates, run_init)
    render/
      mod.rs
      text.rs              (render_overview, render_resource_list, render_resource_detail, render_endpoint_detail,
                            render_schema_list, render_schema_detail, render_auth, render_search)
      json.rs              (same function names as text.rs)
  tests/
    fixtures/
      petstore.yaml
    integration_tests.rs
```

Total tasks: 42 across 14 epics. Estimated at 2-5 minutes each = approximately 1.5-3.5 hours of focused implementation time.
