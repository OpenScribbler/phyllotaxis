# Phyllotaxis

```
        *
       * *
      *   *
     *  *  *
    * *   * *
   *   * *   *
  *  *     *  *
 * *   * *   * *
*   * * * * *   *
```

A CLI for progressive disclosure of OpenAPI specs. Instead of dumping an entire spec at once, phyllotaxis lets you drill down level by level — overview, resources, endpoints, schemas — so you (or an LLM) only see what's relevant. Dual output in plain text and JSON.

## Getting Started

### 1. Clone the repo

```bash
mkdir -p ~/.local/src
git clone https://github.com/holdenhewett/phyllotaxis.git ~/.local/src/phyllotaxis
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
phyllotaxis --help
```

## What It Does

OpenAPI specs are dense. A mid-size API can have hundreds of endpoints, thousands of schema fields, and nested references everywhere. Phyllotaxis applies progressive disclosure — you start with a high-level overview and drill deeper only where you need to.

This matters for LLM-assisted workflows. Instead of stuffing an entire spec into a prompt (blowing token budgets and diluting focus), you feed the LLM exactly the slice it needs: "show me the Pet schema" or "what parameters does POST /pets take?"

## Commands

| Command | Description |
|---------|-------------|
| `phyllotaxis` | API overview — title, base URLs, auth, resource/schema counts |
| `phyllotaxis resources` | List all resource groups with endpoint counts |
| `phyllotaxis resources <name>` | Endpoints within a resource group |
| `phyllotaxis resources <name> <METHOD> <path>` | Full endpoint detail — parameters, request body, responses |
| `phyllotaxis schemas` | List all schemas |
| `phyllotaxis schemas <name>` | Schema detail — fields, types, composition |
| `phyllotaxis auth` | Authentication schemes and usage |
| `phyllotaxis search <term>` | Search across resources, endpoints, and schemas |
| `phyllotaxis init` | Auto-detect spec files and write config |

### Global Flags

```
--spec <path>   Override spec file location
--json          Output in JSON format
--expand        Recursively inline nested schemas (max depth 5)
```

## Progressive Disclosure Levels

### Level 0: Overview

```bash
$ phyllotaxis --spec petstore.yaml
API: Petstore API
Base URL: https://petstore.example.com
Auth: bearerAuth

Commands:
  phyllotaxis resources    List all resource groups (3 available)
  phyllotaxis schemas      List all data models (4 available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

### Level 1: Resource Listing

```bash
$ phyllotaxis resources
Resources:
  pets              Pet management
  deprecated-pets   [DEPRECATED]  Old pet endpoints
  experimental      [ALPHA]       Alpha feature endpoints

Drill deeper:
  phyllotaxis resources <name>
```

### Level 2: Resource Detail

```bash
$ phyllotaxis resources pets
Resource: Pets

Endpoints:
  GET     /pets         List all pets
  POST    /pets         Create a pet
  GET     /pets/{id}    Get a pet by ID
  DELETE  /pets/{id}    Delete a pet

Drill deeper:
  phyllotaxis resources pets GET /pets
```

### Level 3: Endpoint Detail

```bash
$ phyllotaxis resources pets POST /pets
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
```

### Schema Detail

```bash
$ phyllotaxis schemas Pet
Schema: Pet

Fields:
  id        string/uuid  (required, read-only)  Unique identifier
  name      string       (required)             Pet name
  status    string       (optional)             Enum: [available, pending, sold]
  nickname  string       (optional, nullable)   Optional nickname
  owner     Owner        (optional)

Related schemas:
  phyllotaxis schemas Owner
```

### Schema Expansion

```bash
$ phyllotaxis schemas Pet --expand
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

## JSON Output

Every command supports `--json` for machine consumption:

```bash
$ phyllotaxis --json schemas Pet | jq '.fields[].name'
"id"
"name"
"status"
"nickname"
"owner"
```

## Spec Discovery

Phyllotaxis finds your spec file in three ways (in priority order):

1. **`--spec` flag** — explicit path, always wins
2. **`.phyllotaxis.yaml` config** — created by `phyllotaxis init`, checked in the current directory and parents
3. **Auto-detect** — scans for `*.yaml`/`*.yml`/`*.json` files containing `openapi:` in the first 200 bytes

Run `phyllotaxis init` to set up a config:

```bash
$ phyllotaxis init
Detected framework: Astro
Found spec candidates:
  1. ./static/openapi.yaml
Select a spec file (enter number) or type a path: 1
Initialized. Run `phyllotaxis` to see your API overview.
```

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
│   ├── spec.rs              # Config loading, spec resolution, parsing
│   ├── commands/
│   │   ├── overview.rs      # L0: API overview builder
│   │   ├── resources.rs     # L1-L3: resource groups, detail, endpoints
│   │   ├── schemas.rs       # Schema listing, detail, expansion
│   │   ├── auth.rs          # Security scheme extraction
│   │   ├── search.rs        # Cross-type search
│   │   └── init.rs          # Framework detection, interactive setup
│   ├── models/
│   │   ├── resource.rs      # Data structs + utility functions
│   │   └── schema.rs        # SchemaModel, Composition enum
│   └── render/
│       ├── text.rs          # Plain text renderers
│       └── json.rs          # JSON renderers
└── tests/
    ├── fixtures/
    │   └── petstore.yaml    # Test fixture
    ├── fixture_sanity.rs    # Fixture parse validation
    └── integration_tests.rs # End-to-end CLI tests
```

## Development

```bash
cargo build      # Debug build
cargo test       # Run all 72 tests (unit + integration)
cargo build -r   # Release build
```

## License

MIT
