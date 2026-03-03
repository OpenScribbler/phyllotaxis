```
▄▄▄▄▄▄▄   ▄▄          ▄▄ ▄▄
███▀▀███▄ ██          ██ ██        ██              ▀▀
███▄▄███▀ ████▄ ██ ██ ██ ██ ▄███▄ ▀██▀▀ ▀▀█▄ ██ ██ ██  ▄█▀▀▀
███▀▀▀▀   ██ ██ ██▄██ ██ ██ ██ ██  ██  ▄█▀██  ███  ██  ▀███▄
███       ██ ██  ▀██▀ ██ ██ ▀███▀  ██  ▀█▄██ ██ ██ ██▄ ▄▄▄█▀
                  ██
                ▀▀▀
```
A CLI for progressive disclosure of OpenAPI specs. Instead of dumping an entire spec at once, phyllotaxis lets you drill down level by level — overview, resources, endpoints, schemas — so you (or an LLM) only see what's relevant. Dual output in plain text and JSON.

**Alias:** `phyll` — a shorter name for the same binary.

## Getting Started

### 1. Clone the repo

```bash
mkdir -p ~/.local/src
git clone https://github.com/OpenScribbler/phyllotaxis.git ~/.local/src/phyllotaxis
```

### 2. Build the CLI

Requires Rust (install via [rustup](https://rustup.rs/)).

```bash
cd ~/.local/src/phyllotaxis
cargo build --release
```

### 3. Add it to your PATH

```bash
echo 'export PATH="$HOME/.local/src/phyllotaxis/target/release:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

Then verify it works:

```bash
phyll --help
```

## What It Does

OpenAPI specs are dense. A mid-size API can have hundreds of endpoints, thousands of schema fields, and nested references everywhere. Phyllotaxis applies progressive disclosure — you start with a high-level overview and drill deeper only where you need to.

This matters for LLM-assisted workflows. Instead of stuffing an entire spec into a prompt (blowing token budgets and diluting focus), you feed the LLM exactly the slice it needs: "show me the Pet schema" or "what parameters does POST /pets take?"

## Commands

| Command | Description |
|---------|-------------|
| `phyll` | API overview — title, description, base URLs, auth, top resources |
| `phyll resources` | List all resource groups with endpoint counts |
| `phyll resources <name>` | Endpoints within a resource group |
| `phyll resources <name> <METHOD> <path>` | Full endpoint detail — parameters, request body, responses |
| `phyll schemas` | List all schemas |
| `phyll schemas <name>` | Schema detail — fields, types, composition |
| `phyll schemas <name> --used-by` | Which endpoints use this schema in requests, responses, or parameters |
| `phyll schemas <name> --example` | Generate an example JSON object from the schema |
| `phyll auth` | Authentication schemes and usage |
| `phyll search <term>` | Search across resources, endpoints, schemas, security schemes, and callbacks |
| `phyll callbacks` | List all webhook callbacks |
| `phyll callbacks <name>` | Callback detail — operations, URL expressions, schemas |
| `phyll init` | Auto-detect spec files and write config |
| `phyll completions <shell>` | Generate shell completions (bash, zsh, fish, powershell, elvish) |

### Global Flags

```
--spec <name|path>           Override spec file (named spec from config, or file path)
--json                       Output in JSON format
--expand                     Recursively inline nested schemas (max depth 5)
--related-limit <n>          Cap the number of related schemas shown in schema detail
```

### Endpoint Detail Flags

```
--context     Show related schemas expanded after the endpoint detail
--example     Show an auto-generated example request/response body
```

## Progressive Disclosure Levels

### Level 0: Overview

```bash
$ phyll --spec petstore.yaml
API: Petstore API
Base URL: https://petstore.example.com
Auth: bearerAuth

Top Resources:
  pets                     (4 endpoints)
  deprecated-pets          (2 endpoints)

Commands:
  phyll resources    List all resource groups (3 available)
  phyll schemas      List all data models (4 available)
  phyll auth         Authentication details
  phyll search       Search across all endpoints and schemas
```

### Level 1: Resource Listing

```bash
$ phyll resources
Resources:
  pets              Pet management
  deprecated-pets   [DEPRECATED]  Old pet endpoints
  experimental      [ALPHA]       Alpha feature endpoints

Drill deeper:
  phyll resources <name>
```

### Level 2: Resource Detail

```bash
$ phyll resources pets
Resource: Pets

Endpoints:
  GET     /pets         List all pets
  POST    /pets         Create a pet
  GET     /pets/{id}    Get a pet by ID
  DELETE  /pets/{id}    Delete a pet

Drill deeper:
  phyll resources pets GET /pets
```

### Level 3: Endpoint Detail

```bash
$ phyll resources pets POST /pets
POST /pets

Authentication: bearerAuth (required)

Request Body (application/json):
  name      string       (required)              Pet name

Request Example:
  { "name": "Fido" }

Responses:
  201 Created → Pet

Errors:
  400 Invalid input
  409 Duplicate pet

Drill deeper:
  phyll schemas Pet
```

### Schema Detail

```bash
$ phyll schemas Pet
Schema: Pet

Fields:
  id        string/uuid  (required, read-only)  Unique identifier
  name      string       (required)             Pet name
  status    string       (optional)             Enum: [available, pending, sold]
  nickname  string       (optional, nullable)   Optional nickname
  owner     Owner        (optional)

Related schemas:
  phyll schemas Owner
```

### Callbacks

```bash
$ phyll callbacks
Callbacks:
  onPetAdded    Defined on: POST /pets

Drill deeper:
  phyll callbacks <name>
```

```bash
$ phyll callbacks onPetAdded
Callback: onPetAdded
Defined on: POST /pets

Operations:
  POST {$request.body#/callbackUrl}
    Body: PetEvent
    Responses:
      200 Callback received
```

## Example Generation

Generate example JSON objects from any schema, with format-aware placeholders:

```bash
$ phyll schemas Pet --example
Example (Pet, required fields, auto-generated):
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "string"
}
```

Examples use intelligent placeholders based on field types and formats:

| Type/Format | Placeholder |
|-------------|-------------|
| `string` | `"string"` |
| `string/uuid` | `"550e8400-e29b-41d4-a716-446655440000"` |
| `string/date-time` | `"2024-01-15T10:30:00Z"` |
| `string/email` | `"user@example.com"` |
| `string/uri` | `"https://example.com"` |
| `integer` | `0` |
| `boolean` | `true` |
| enum | First enum value |

When the spec includes `example` values on schemas or properties, those are used instead of placeholders. For discriminated unions (oneOf with a discriminator), the `type` field is set to the correct mapped value.

## Reverse Schema Lookup

Find which endpoints use a specific schema:

```bash
$ phyll schemas TagDTO --used-by
Schema: TagDTO

Used by 114 endpoint(s):

  In request body:
    POST    /api/v1/access-conditions
    PUT     /api/v1/access-conditions
    ...

  In response:
    GET     /api/v1/access-conditions/{id}
    ...
```

Matches include direct `$ref` references, composition variants (allOf/oneOf/anyOf), and transitive field-type references (e.g., a schema embedded as a field inside another schema that an endpoint uses).

## Related Schemas (--context)

When viewing endpoint detail, `--context` expands the nested schemas referenced by the request/response body:

```bash
$ phyll resources access-policy-v2 POST /api/v2/access-policies --context
POST /api/v2/access-policies
...

Related Schemas:

  TagDTO (Aembit Entity Tag Details):
  key    string  (required)  Tag Key
  value  string  (required)  Tag Key Value

  PolicyCredentialMappingDTO (Access Policy Credential Mappings):
  credentialProviderId  string/uuid  (required)
  mappingType           enum         (required)  [None, AccountName, HttpHeader, HttpBody]
  ...
```

For polymorphic endpoints (oneOf/anyOf), `--context` shows the variant schemas.

## Schema Expansion

```bash
$ phyll schemas Pet --expand
Schema: Pet (expanded)

Fields:
  id        string/uuid  (required, read-only)  Unique identifier
  name      string       (required)             Pet name
  status    string       (optional)             Enum: [available, pending, sold]
  nickname  string       (optional, nullable)   Optional nickname
  owner     Owner:
    id    string  (read-only)   Owner identifier
    name  string                Owner name
```

## Search

Search across resources, endpoints, schemas, security schemes, and callbacks:

```bash
$ phyll search "authentication"
```

Search indexes: resource names/descriptions, endpoint paths/summaries/descriptions, parameter names/descriptions, request body descriptions, response descriptions, schema names/descriptions/field names, and security scheme names/descriptions.

When a match comes from a non-obvious source (parameter name, description text, security scheme), the result is annotated with the match reason.

## JSON Output

Every command supports `--json` for machine consumption. JSON is pretty-printed in a terminal and compact when piped:

```bash
$ phyll --json schemas Pet | jq '.fields[].name'
"id"
"name"
"status"
"nickname"
"owner"
```

## Fuzzy Matching

Mistype a resource, schema, or callback name and phyllotaxis suggests close matches:

```bash
$ phyll resources pet
Error: Resource 'pet' not found.
Did you mean:
  phyll resources pets
```

## Helpful Error Messages

Pass a method and path as a single quoted argument and phyllotaxis detects the mistake:

```bash
$ phyll resources pets "GET /pets"
Error: Method and path must be separate arguments.

  You passed:  "GET /pets"
  Try instead: phyll resources pets GET /pets
```

## Spec Discovery

Phyllotaxis finds your spec file in four ways (in priority order):

1. **`--spec` flag** — named spec from config or file path, always wins
2. **`PHYLLOTAXIS_SPEC` env var** — set to a file path; errors if set but the file doesn't exist, silently ignored if empty
3. **`.phyllotaxis.yaml` config** — created by `phyll init`, checked in the current directory and parents
4. **Auto-detect** — scans for `*.yaml`/`*.yml`/`*.json` files containing `openapi:` in the first 200 bytes

Run `phyll init` to set up a config:

```bash
$ phyll init
Detected framework: Astro
Found spec candidates:
  1. ./static/openapi.yaml
Select a spec file (enter number) or type a path: 1
Initialized. Run `phyll` to see your API overview.
```

For non-interactive setup (CI, scripts), pass the path directly:

```bash
$ phyll init --spec-path ./api/openapi.yaml
```

### Multi-Spec Projects

If your project has multiple API specs, use named specs in `.phyllotaxis.yaml`:

```yaml
specs:
  public: ./api/public.yaml
  internal: ./api/internal.yaml
default: public
variables:
  tenant: my-org
  env: staging
```

Then select a spec by name:

```bash
$ phyll --spec internal resources
```

The `variables` map substitutes server URL template variables (e.g., `{tenant}` becomes `my-org` in base URL output).

## Compatibility

- **OpenAPI 3.0.x** — fully supported
- **OpenAPI 3.1** — not supported (the `openapiv3` parser targets 3.0)
- **Swagger / OpenAPI 2.0** — not supported
- **YAML and JSON specs** — both work
- **`$ref` resolution** — local references only (no external file refs)

## Project Structure

```
phyllotaxis/
├── src/
│   ├── main.rs              # CLI entry point (clap)
│   ├── lib.rs               # Public crate API (re-exports)
│   ├── spec.rs              # Config loading, spec resolution, parsing
│   ├── commands/
│   │   ├── overview.rs      # L0: API overview builder
│   │   ├── resources.rs     # L1-L3: resource groups, detail, endpoints
│   │   ├── schemas.rs       # Schema listing, detail, expansion, --used-by
│   │   ├── examples.rs      # Example generation from schemas
│   │   ├── auth.rs          # Security scheme extraction
│   │   ├── search.rs        # Cross-type search
│   │   ├── callbacks.rs     # Webhook callback extraction
│   │   └── init.rs          # Framework detection, interactive setup
│   ├── models/
│   │   ├── resource.rs      # Data structs + utility functions
│   │   └── schema.rs        # SchemaModel, Composition enum
│   └── render/
│       ├── text.rs          # Plain text renderers
│       └── json.rs          # JSON renderers
└── tests/
    ├── fixtures/
    │   ├── petstore.yaml    # Test fixture
    │   └── kitchen-sink.yaml # Comprehensive edge-case fixture
    ├── fixture_sanity.rs    # Fixture parse validation
    ├── integration_tests.rs # End-to-end CLI tests
    └── lib_tests.rs         # Library API tests
```

## Development

```bash
cargo build      # Debug build
cargo test       # Run all tests (unit + integration)
cargo clippy     # Lint
cargo build -r   # Release build
```

## License

MIT
