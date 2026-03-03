```
▄▄▄▄▄▄▄   ▄▄          ▄▄ ▄▄
███▀▀███▄ ██          ██ ██        ██              ▀▀
███▄▄███▀ ████▄ ██ ██ ██ ██ ▄███▄ ▀██▀▀ ▀▀█▄ ██ ██ ██  ▄█▀▀▀
███▀▀▀▀   ██ ██ ██▄██ ██ ██ ██ ██  ██  ▄█▀██  ███  ██  ▀███▄
███       ██ ██  ▀██▀ ██ ██ ▀███▀  ██  ▀█▄██ ██ ██ ██▄ ▄▄▄█▀
                  ██
                ▀▀▀
```
A CLI that lets you explore OpenAPI specs one layer at a time instead of reading the whole thing. Start with an overview, pick a resource, drill into an endpoint, check a schema. You (or an LLM) only see what you actually need.

Outputs plain text or JSON. Also available as `phyll` (shorter alias, same binary).

## Install

### Download a binary (recommended)

Grab the latest release for your platform from [GitHub Releases](https://github.com/OpenScribbler/phyllotaxis/releases/latest).

**Linux / macOS:**

```bash
# Pick your platform:
#   x86_64-unknown-linux-gnu    (Linux x86_64)
#   aarch64-unknown-linux-gnu   (Linux ARM64)
#   x86_64-apple-darwin         (macOS Intel)
#   aarch64-apple-darwin        (macOS Apple Silicon)
PLATFORM="x86_64-unknown-linux-gnu"

curl -L "https://github.com/OpenScribbler/phyllotaxis/releases/latest/download/phyllotaxis-${PLATFORM}.tar.gz" \
  | tar xz -C ~/.local/bin
```

**Windows:**

Download the `phyllotaxis-x86_64-pc-windows-msvc.zip` from the [releases page](https://github.com/OpenScribbler/phyllotaxis/releases/latest), extract it, and add the folder to your PATH.

### Build from source

Requires Rust (install via [rustup](https://rustup.rs/)).

```bash
git clone https://github.com/OpenScribbler/phyllotaxis.git
cd phyllotaxis
cargo build --release
# Binaries are in target/release/phyllotaxis and target/release/phyll
```

### Verify

```bash
phyll --help
```

## Why?

OpenAPI specs get big fast. A mid-size API can have hundreds of endpoints, thousands of schema fields, and nested `$ref`s everywhere. Phyllotaxis gives you layers: start high, go deep only where you care.

This is especially useful for LLM workflows. Instead of stuffing an entire spec into a prompt and burning tokens, you can give the model just the slice it needs: "show me the Pet schema" or "what does POST /pets expect?"

## Commands

| Command | What it shows |
|---------|-------------|
| `phyll` | API overview: title, description, base URLs, auth, top resources |
| `phyll resources` | All resource groups with endpoint counts |
| `phyll resources <name>` | Endpoints in a resource group |
| `phyll resources <name> <METHOD> <path>` | Full endpoint detail: params, request body, responses |
| `phyll schemas` | All schemas |
| `phyll schemas <name>` | Schema detail: fields, types, composition |
| `phyll schemas <name> --used-by` | Which endpoints use this schema |
| `phyll schemas <name> --example` | Generate an example JSON object |
| `phyll auth` | Auth schemes and how they're used |
| `phyll search <term>` | Search across everything |
| `phyll callbacks` | Webhook callbacks |
| `phyll callbacks <name>` | Callback detail |
| `phyll init` | Auto-detect spec files and write config |
| `phyll completions <shell>` | Shell completions (bash, zsh, fish, powershell, elvish) |

### Global Flags

```
--spec <name|path>           Override which spec file to use
--json                       Output JSON instead of text
--expand                     Inline nested schemas recursively (max depth 5)
--related-limit <n>          Cap how many related schemas to show
```

### Endpoint Detail Flags

```
--context     Show related schemas expanded after the endpoint
--example     Show an auto-generated example request/response body
```

## How the Layers Work

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

Generate example JSON from any schema:

```bash
$ phyll schemas Pet --example
Example (Pet, required fields, auto-generated):
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "string"
}
```

Placeholders are based on the field type and format:

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

If the spec has `example` values on schemas or properties, those get used instead. For discriminated unions (oneOf with a discriminator), the `type` field gets set to the correct mapped value.

## Reverse Schema Lookup

Find out which endpoints use a given schema:

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

This catches direct `$ref` references, allOf/oneOf/anyOf compositions, and schemas nested as fields inside other schemas that an endpoint uses.

## Related Schemas (--context)

When you're looking at an endpoint, `--context` expands the schemas referenced in the request/response body:

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

For oneOf/anyOf endpoints, `--context` shows the variant schemas too.

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

Searches resource names/descriptions, endpoint paths/summaries/descriptions, parameter names/descriptions, request body descriptions, response descriptions, schema names/descriptions/field names, and security scheme names/descriptions.

If a match comes from somewhere non-obvious (like a parameter name or description text), the result tells you why it matched.

## JSON Output

Every command supports `--json`. It's pretty-printed in a terminal and compact when piped:

```bash
$ phyll --json schemas Pet | jq '.fields[].name'
"id"
"name"
"status"
"nickname"
"owner"
```

## Fuzzy Matching

Mistype a name and phyllotaxis suggests close matches:

```bash
$ phyll resources pet
Error: Resource 'pet' not found.
Did you mean:
  phyll resources pets
```

## Helpful Error Messages

If you accidentally pass the method and path as a single quoted argument, phyllotaxis catches it:

```bash
$ phyll resources pets "GET /pets"
Error: Method and path must be separate arguments.

  You passed:  "GET /pets"
  Try instead: phyll resources pets GET /pets
```

## Spec Discovery

Phyllotaxis finds your spec file in four ways (checked in this order):

1. **`--spec` flag** - named spec from config or a file path, always wins
2. **`PHYLLOTAXIS_SPEC` env var** - set to a file path; errors if the file doesn't exist, ignored if empty
3. **`.phyllotaxis.yaml` config** - created by `phyll init`, checked in the current directory and parents
4. **Auto-detect** - scans for `*.yaml`/`*.yml`/`*.json` files with `openapi:` in the first 200 bytes

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

Then pick one by name:

```bash
$ phyll --spec internal resources
```

The `variables` map fills in server URL template variables (e.g., `{tenant}` becomes `my-org` in base URL output).

## Compatibility

- **OpenAPI 3.0.x** - fully supported
- **OpenAPI 3.1** - not supported (the `openapiv3` parser targets 3.0)
- **Swagger / OpenAPI 2.0** - not supported
- **YAML and JSON specs** - both work
- **`$ref` resolution** - local references only, no external file refs

## Project Structure

```
phyllotaxis/
├── src/
│   ├── main.rs              # CLI entry point (clap)
│   ├── lib.rs               # Public crate API (re-exports)
│   ├── spec.rs              # Config loading, spec resolution, parsing
│   ├── commands/
│   │   ├── overview.rs      # L0: API overview
│   │   ├── resources.rs     # L1-L3: resource groups, detail, endpoints
│   │   ├── schemas.rs       # Schema listing, detail, expansion, --used-by
│   │   ├── examples.rs      # Example generation from schemas
│   │   ├── auth.rs          # Security scheme extraction
│   │   ├── search.rs        # Cross-type search
│   │   ├── callbacks.rs     # Webhook callback extraction
│   │   └── init.rs          # Framework detection, interactive setup
│   ├── models/
│   │   ├── resource.rs      # Data structs + helpers
│   │   └── schema.rs        # SchemaModel, Composition enum
│   └── render/
│       ├── text.rs          # Plain text output
│       └── json.rs          # JSON output
└── tests/
    ├── fixtures/
    │   ├── petstore.yaml    # Test fixture
    │   └── kitchen-sink.yaml # Edge case fixture
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
