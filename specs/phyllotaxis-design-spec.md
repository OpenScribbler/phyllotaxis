# Phyllotaxis — Design Spec

> Progressive disclosure for OpenAPI specs. Built for LLM agents.

## Name

**Phyllotaxis** — from Ancient Greek *phullon* (leaf) + *taxis* (arrangement/order).

In botany, phyllotaxis is the mathematical pattern governing how leaves and florets are arranged on a plant stem. It's the algorithm behind Romanesco's fractal spirals — each element positioned at the optimal angle following the Fibonacci sequence.

The metaphor: this tool reveals the organized arrangement of an API's structure, navigable by pattern. Not about growing or generating — about navigating what's already there, in the right order.

- **crates.io:** `phyllotaxis` — available
- **Romanesco family:** Phyllotaxis is the math behind the fractal spiral
- **Primary consumer:** LLM agents (spelling/length is a non-issue)

## Concept

A Rust CLI binary that gives LLM agents a progressive disclosure interface to an OpenAPI spec. Instead of dumping an entire spec into context, LLM agents explore documentation incrementally — starting broad and drilling into exactly what they need.

## Core Principles

- **LLM-first consumer.** Optimized for token efficiency and machine readability, not human aesthetics.
- **Progressive disclosure.** Every level of output reveals just enough to act or decide where to drill deeper.
- **Zero friction.** Single binary, no runtime dependencies, no configuration beyond pointing at a spec.
- **Self-guiding.** Every output includes hints showing the next available commands.

## Setup & Spec Discovery

### `phyllotaxis init`

Interactive setup command that detects the user's documentation framework, searches common paths for the OpenAPI spec, and writes configuration.

**Flow:**

1. Detect doc framework by signature files:
   - `astro.config.mjs` -> Astro Starlight -> search `src/content/`, `public/`
   - `docusaurus.config.js` -> Docusaurus -> search `static/`, `docs/`
   - `hugo.toml` / `config.toml` -> Hugo -> search `static/`, `content/`
   - `_config.yml` -> Jekyll -> search `_data/`, `assets/`
   - `mkdocs.yml` -> MkDocs -> search `docs/`
2. Search detected paths for files matching `openapi.{yaml,yml,json}` or `swagger.{yaml,yml,json}`. Optionally peek inside YAML/JSON files for `openapi: "3.x"` key to catch specs with non-obvious filenames.
3. If found, confirm with user: "Is this your OpenAPI specification file? (y/n)"
4. If not found, prompt for manual path entry.
5. Write `.phyllotaxis.yaml` config file to project root.

### Spec Discovery Priority

When running any command, the CLI resolves the spec location in this order:

1. `--spec` flag (explicit always wins)
2. `.phyllotaxis.yaml` config file (walk up directory tree)
3. Auto-detect `openapi.yaml` / `openapi.json` in current directory
4. Error with helpful message

### Config File (`.phyllotaxis.yaml`)

```yaml
spec: ./path/to/openapi.yaml
```

Committed to the repo so any user or LLM agent cloning the repo gets the CLI working immediately.

## Command Structure

The CLI uses nested progressive disclosure — you keep adding arguments to drill deeper. Every level's output naturally hints at the next level.

```
phyllotaxis                                       -> API overview + available commands
phyllotaxis resources                             -> List all resource groups
phyllotaxis resources <name>                      -> Endpoints for that resource
phyllotaxis resources <name> <METHOD> <path>      -> Full endpoint detail
phyllotaxis schemas                               -> List all models
phyllotaxis schemas <name>                        -> Full model definition
phyllotaxis auth                                  -> Authentication details
phyllotaxis search <term>                         -> Search across everything
```

### Global Flags

- `--spec <path>` — Override spec file location
- `--json` — Output in JSON format (escape hatch for programmatic use)
- `--expand` — For schema views, recursively inline all nested schema definitions instead of showing one level with drill-deeper hints

## Output Design

### Principles

- Plain text by default. No ANSI colors, no box-drawing characters.
- Token-efficient. Terse descriptions over verbose ones.
- Every output ends with "drill deeper" hints showing exact commands for the next level.
- Examples from the spec are included when present, omitted when not.

### Output by Level

**Level 0 — `phyllotaxis`**

```
API: Aembit Cloud API
Base URL: https://api.aembit.io/v1
Auth: Bearer token (OAuth2 client credentials)

Commands:
  phyllotaxis resources    List all resource groups (14 available)
  phyllotaxis schemas      List all data models (32 available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

**Level 1 — `phyllotaxis resources`**

```
Resources:
  access-policies       Define which client workloads can access server workloads
  client-workloads      Workloads that initiate access requests
  server-workloads      Target workloads that receive access requests
  trust-providers       External identity providers for workload attestation
  ...

Drill deeper:
  phyllotaxis resources <name>
```

**Level 2 — `phyllotaxis resources access-policies`**

```
Resource: Access Policies
Description: Define which client workloads can access server workloads.

Endpoints:
  GET    /access-policies           List all access policies
  POST   /access-policies           Create an access policy
  GET    /access-policies/{id}      Get a specific access policy
  PUT    /access-policies/{id}      Update an access policy
  DELETE /access-policies/{id}      Delete an access policy

Drill deeper:
  phyllotaxis resources access-policies GET /access-policies
  phyllotaxis resources access-policies POST /access-policies
```

**Level 3 — `phyllotaxis resources access-policies POST /access-policies`**

```
POST /access-policies
Create a new access policy.

Authentication: Bearer token (required)

Request Body (application/json):
  name              string   (required)  Display name for the policy
  clientWorkloadId  string   (required)  ID of the client workload
  serverWorkloadId  string   (required)  ID of the target server workload
  isActive          boolean  (optional)  Whether the policy is active. Default: true
  conditions        object   (optional)  Additional conditions for policy evaluation

Response: 201 Created
  Returns: AccessPolicy object

Errors:
  400  Invalid request body
  401  Unauthorized
  409  Policy with this name already exists

Drill deeper:
  phyllotaxis schemas AccessPolicy
  phyllotaxis schemas PolicyConditions
```

**Schema view — `phyllotaxis schemas AccessPolicy`**

```
Schema: AccessPolicy

Fields:
  id                string     (read-only)  Unique identifier
  name              string     (required)   Display name for the policy
  clientWorkloadId  string     (required)   ID of the client workload
  serverWorkloadId  string     (required)   ID of the target server workload
  isActive          boolean                 Whether the policy is active. Default: true
  conditions        PolicyConditions        Additional conditions for policy evaluation
  createdAt         datetime   (read-only)  When the policy was created
  modifiedAt        datetime   (read-only)  When the policy was last modified

Related:
  phyllotaxis schemas PolicyConditions
  phyllotaxis resources access-policies
```

## Technical Implementation

### Language & Distribution

- **Language:** Rust
- **CLI framework:** `clap` (nested subcommand support)
- **OpenAPI parsing:** `oas3` or `openapiv3` crate
- **Distribution:** Compiled binaries per OS/arch via GitHub releases
- **Installation:** Direct download, `brew install`, `cargo install`

### Architecture

The CLI is stateless — every invocation reads the spec, resolves the requested command, and prints output. No daemon, no cache, no build step. The spec is the database.

## Deprecation Handling

Resources tagged as deprecated in the spec (e.g. `Access Policy (Deprecated)`) are shown in listings but clearly marked:

```
Resources:
  access-condition           Aembit Access Conditions for policy evaluation
  access-policy-v2           Access policies with credential mappings
  access-policy              [DEPRECATED -> use access-policy-v2] Access policies
  credential-provider-v2     Credential providers for workload authentication
  credential-provider        [DEPRECATED -> use credential-provider-v2] Credential providers
  ...
```

When an LLM drills into a deprecated resource, the output includes a warning at the top:

```
WARNING: DEPRECATED. Use "access-policy-v2" instead.
  phyllotaxis resources access-policy-v2

Resource: Access Policy
...
```

## Resource Name Slugification

OpenAPI tags become CLI-friendly slugs automatically:

- Lowercase, spaces to hyphens
- `(Deprecated)` stripped from slug (handled by deprecation marking)
- PascalCase split (e.g. `DiscoveryIntegration` -> `discovery-integration`)

Examples:
```
"Access Condition"                -> access-condition
"Access Policy v2"                -> access-policy-v2
"Access Policy (Deprecated)"      -> access-policy
"DiscoveryIntegration"            -> discovery-integration
"MFA SignOn Policy"               -> mfa-signon-policy
```

The original display name is preserved in output. Slugs are only used as CLI arguments.

## Search

`phyllotaxis search <term>` performs case-insensitive substring matching across:

- Tag names and descriptions
- Endpoint paths and summaries
- Schema names
- Operation IDs

Results are grouped by type with drill-deeper hints:

```
Results for "workload":

Resources:
  client-workload              Workloads that initiate access requests
  server-workload              Target workloads that receive access requests

Endpoints:
  GET  /api/v1/client-workloads              List all client workloads
  GET  /api/v1/server-workloads              List all server workloads
  ...

Schemas:
  ClientWorkloadExternalDTO
  ClientWorkloadListDTO
  ...

Drill deeper:
  phyllotaxis resources client-workload
  phyllotaxis schemas ClientWorkloadExternalDTO
```

Simple substring matching for POC. Upgrade to fuzzy matching later if needed.

## Nested Schema Handling

Schemas are displayed one level deep by default. Fields that reference other schemas show the schema name as the type, with drill-deeper hints for each referenced schema:

```
Schema: AccessPolicyV2DTO

Fields:
  id                  string                  (read-only)  Unique identifier
  name                string                  (required)   Display name
  clientWorkload      EntityMetaDTO                        The client workload
  credentialMappings  CredentialMappingDTO[]                Credential mappings

Related schemas:
  phyllotaxis schemas EntityMetaDTO
  phyllotaxis schemas CredentialMappingDTO
```

With `--expand`, all referenced schemas are recursively inlined:

```
Schema: AccessPolicyV2DTO (expanded)

Fields:
  id                  string                  (read-only)  Unique identifier
  name                string                  (required)   Display name
  clientWorkload      EntityMetaDTO:
    id                  string                (read-only)  Unique identifier
    name                string                             Display name
  credentialMappings  CredentialMappingDTO[]:
    credentialProviderId  string              (required)   ID of the credential provider
    ...
```

## Target Spec Profile (Aembit Cloud API)

Analysis of the actual spec that will be used for the POC:

- **Spec version:** OpenAPI 3.0.4
- **Resource groups (tags):** 31 — including 2 deprecated, 1 alpha
- **Paths:** 66
- **Operations:** 142 (61 GET, 23 POST, 22 PUT, 19 PATCH, 17 DELETE)
- **Schemas:** 143
- **Auth:** Bearer token (HTTP bearer scheme)
- **API versions:** Mostly v1, some v2 replacements, one alpha endpoint
- **Description richness:** Summaries present on most operations but terse. Schema field descriptions vary.
- **Base URL:** `https://{tenant}.aembit-eng.com` (tenant-scoped)

## Proof of Concept Scope

- **Target spec:** Aembit's actual OpenAPI specification
- **Goal:** Validate that progressive disclosure via CLI meaningfully improves how LLM agents navigate and consume API documentation
- **Out of scope for POC:** User docs integration, doc-to-spec association, custom content injection