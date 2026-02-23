# Phyllotaxis — Design Document

**Goal:** Build a Rust CLI that gives LLM agents a progressive disclosure interface to OpenAPI specs, optimized for token efficiency and machine readability.

**Decision Date:** 2026-02-20

---

## Problem Statement

LLM agents working with APIs must understand the API structure before making requests. Dumping an entire OpenAPI spec into context wastes tokens and overwhelms the agent. There's no tool that gives LLMs an incremental, self-guiding way to explore an API — starting broad and drilling into exactly what they need.

## Proposed Solution

A stateless Rust CLI binary (`phyllotaxis`) that parses an OpenAPI spec and exposes it through nested progressive disclosure. Commands add arguments to drill deeper. Every level's output hints at the next level. Plain text by default, JSON via flag.

## Architecture

### Core Flow

Every invocation follows the same path:

1. Resolve spec location (flag → config → auto-detect → error)
2. Parse spec with `openapiv3` crate
3. Route to command handler based on CLI args
4. Render output (plain text or JSON)
5. Exit

No daemon, no cache, no state. The spec is the database.

### Module Structure

```
src/
  main.rs           CLI entry point, clap setup
  spec.rs           Spec loading, parsing, config resolution
  commands/
    overview.rs     Level 0 — API overview
    resources.rs    Levels 1-3 — resource listing, endpoints, endpoint detail
    schemas.rs      Schema display, expansion, composition flattening
    auth.rs         Authentication details
    search.rs       Cross-type search
    init.rs         Interactive setup
  render/
    text.rs         Plain text formatter
    json.rs         JSON formatter
  models/
    resource.rs     Intermediate resource/endpoint models
    schema.rs       Intermediate schema models with ref tracking
```

### Dependencies

- **CLI framework:** `clap` (derive mode, nested subcommands)
- **OpenAPI parsing:** `openapiv3` v2.2.0 (built for OpenAPI 3.0.x, stable, no recursive schema crash)
- **YAML/JSON:** `serde_yaml`, `serde_json`
- **Config:** `serde` for `.phyllotaxis.yaml` deserialization

## Key Decisions

| # | Decision | Choice | Reasoning |
|---|----------|--------|-----------|
| 1 | Parser crate | `openapiv3` | Built for OpenAPI 3.0.x (matches target spec 3.0.4). Stable v2.2.0. `oas3` targets 3.1, crashes on recursive schemas, and is pre-1.0. |
| 2 | `auth` command output | Mirror spec's security schemes | Show scheme type, details, usage count. Lean output that scales to complex multi-scheme specs. |
| 3 | `--json` output | Structured progressive disclosure | JSON version of the curated output, not raw spec pass-through. Same data as plain text, different format. |
| 4 | Level 3 parameters | Show all types (path, query, header) | Grouped by type. LLM needs everything to construct a full request. Parameters merge from path-level and operation-level per OpenAPI spec. |
| 5 | `--expand` safety | Depth limit (5) + cycle detection | Cycle detection marks `[circular: SchemaName]`. Depth 5 covers real-world nesting (typically 3-5 levels). Research: Swagger UI defaults to 1, Redocly recommends 8 for generation. |
| 6 | `init` command | In POC scope | Full framework detection and interactive setup. First-run polish matters for adoption. |
| 7 | Deprecation markers | `[DEPRECATED]` only — no replacement hint | Simple marker in listings, warning when drilling in. LLM can see the resource list and infer replacements. |
| 8 | Alpha/beta markers | `[ALPHA]` same pattern as deprecated | Consistent treatment for non-stable endpoints. Marker in listings, warning on drill-in. |
| 9 | Server URL variables | Show template + details, resolve with config | Default: show `https://{tenant}.example.com` with variable descriptions below. Config overrides in `.phyllotaxis.yaml` resolve variables. |
| 10 | Enums and examples | Inline enums, inline short examples, separate body examples | Enum values inline with field (e.g., `Enum: [active, inactive]`). Short examples inline. Full request/response body examples in separate sections. Optimized for LLM field-by-field construction. |
| 11 | Spec parsing strategy | Stateless — parse every invocation | Rust YAML parsing is fast. Avoids cache invalidation complexity. Revisit only if profiling shows bottleneck. |
| 12 | Search ranking | Flat list, grouped by type | Groups: Resources → Endpoints → Schemas, fixed order. No ranking within groups. LLMs scan all matches in one pass. |
| 13 | Schema composition | Flatten `allOf`, label `oneOf`/`anyOf` | `allOf` merges into single flat field list (semantic meaning). `oneOf`/`anyOf` show "One of:" / "Any of:" with variant links. |
| 14 | Level 3 response body | Reference + example | Schema name with drill-deeper hint (not inline fields). Response example at bottom when spec provides one. Keeps Level 3 focused on request construction. |
| 15 | Field format info | Show as type/format | Display `string/uuid`, `string/date-time`, etc. when format is specified. Tells LLM the exact value shape to generate. |
| 16 | Nullable marking | Show alongside optional | Mark `(nullable)` or `(optional, nullable)` on fields. Matters for PATCH where `null` = clear vs omit = no change. |
| 17 | Spec documentation | Summary in listings, full in detail | Operation `summary` in Level 1-2 listings. Full `description` in Level 3 (truncated at ~500 chars). Empty descriptions omitted. `externalDocs` as "See also:" links. |

## Command Reference

### `phyllotaxis` (Level 0 — Overview)

```
API: Aembit Cloud API
Base URL: https://{tenant}.aembit-eng.com
  Variables:
    tenant  (required)  Your Aembit tenant name
Auth: Bearer token (HTTP bearer)

Commands:
  phyllotaxis resources    List all resource groups (31 available)
  phyllotaxis schemas      List all data models (143 available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

With config override (`variables: { tenant: acme-corp }`):
```
Base URL: https://acme-corp.aembit-eng.com
```

### `phyllotaxis resources` (Level 1)

```
Resources:
  access-condition           Aembit Access Conditions for policy evaluation
  access-policy-v2           Access policies with credential mappings
  access-policy              [DEPRECATED] Access policies
  credential-provider-v2     Credential providers for workload authentication
  credential-provider        [DEPRECATED] Credential providers
  discovery-integration      [ALPHA] Discovery integration endpoints
  ...

Drill deeper:
  phyllotaxis resources <name>
```

### `phyllotaxis resources <name>` (Level 2)

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

### `phyllotaxis resources <name> <METHOD> <path>` (Level 3)

```
POST /access-policies
Create a new access policy.

Authentication: Bearer token (required)

Path Parameters:
  (none)

Query Parameters:
  (none)

Request Body (application/json):
  name              string   (required)  Display name for the policy
  clientWorkloadId  string   (required)  ID of the client workload
  serverWorkloadId  string   (required)  ID of the target server workload
  isActive          boolean              Whether the policy is active. Default: true
  conditions        object               Additional conditions for policy evaluation

Request Example:
  {
    "name": "My Policy",
    "clientWorkloadId": "abc-123",
    "serverWorkloadId": "def-456"
  }

Response: 201 Created
  Returns: AccessPolicy

Response Example:
  {
    "id": "policy-789",
    "name": "My Policy",
    "clientWorkloadId": "abc-123",
    "serverWorkloadId": "def-456",
    "isActive": true,
    "createdAt": "2024-01-15T00:00:00Z"
  }

Errors:
  400  Invalid request body
  401  Unauthorized
  409  Policy with this name already exists

Drill deeper:
  phyllotaxis schemas AccessPolicy
  phyllotaxis schemas PolicyConditions
```

### `phyllotaxis schemas <name>` (Default — one level)

```
Schema: AccessPolicyV2DTO

Fields:
  id                  string/uuid             (read-only)  Unique identifier
  name                string                  (required)   Display name
  status              string                               Enum: [active, inactive, pending]
  nickname            string                  (optional, nullable)  Can be cleared with null
  clientWorkload      EntityMetaDTO                        The client workload
  credentialMappings  CredentialMappingDTO[]                Credential mappings
  createdAt           string/date-time        (read-only)  When the policy was created

Related schemas:
  phyllotaxis schemas EntityMetaDTO
  phyllotaxis schemas CredentialMappingDTO
```

### `phyllotaxis schemas <name> --expand` (Recursive expansion)

Max depth: 5. Cycle detection: `[circular: SchemaName]`.

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

### `phyllotaxis auth`

```
Authentication:
  bearer (HTTP)
    Scheme: bearer
    Description: JWT token for API access

  Used by: 142 operations (all endpoints)

Drill deeper:
  phyllotaxis resources    Browse endpoints by resource group
```

### `phyllotaxis search <term>`

Case-insensitive substring matching. Results grouped by type, no ranking.

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

### `phyllotaxis init`

Interactive setup. In POC scope.

1. Detect doc framework by signature files (Astro, Docusaurus, Hugo, Jekyll, MkDocs)
2. Search detected paths for OpenAPI spec files
3. Confirm or prompt for manual path
4. Write `.phyllotaxis.yaml`

### Global Flags

| Flag | Description |
|------|-------------|
| `--spec <path>` | Override spec file location |
| `--json` | Output as structured JSON (same data as plain text, machine-readable format) |
| `--expand` | Recursively inline nested schemas (max depth 5, cycle detection) |

## Config File (`.phyllotaxis.yaml`)

```yaml
spec: ./path/to/openapi.yaml
variables:
  tenant: acme-corp
```

Committed to repo. Variables section is optional — used to resolve server URL templates.

## Data Flow

```
CLI args → clap parsing → spec resolution → openapiv3 parsing → command routing → rendering → stdout
```

1. **Spec resolution:** `--spec` flag > `.phyllotaxis.yaml` (walk up dirs) > auto-detect > error
2. **Parsing:** `openapiv3` deserializes YAML/JSON into typed Rust structs
3. **Ref resolution:** Manual — `ReferenceOr<T>` enum. Resolve on demand, not eagerly.
4. **Rendering:** Plain text (default) or JSON. Both share the same intermediate model.

## Schema Composition Handling

| Keyword | Rendering |
|---------|-----------|
| `allOf` | Flatten — merge all referenced schemas into single field list |
| `oneOf` | Label — "One of:" with links to each variant schema |
| `anyOf` | Label — "Any of:" with links to each variant schema |

## Resource Name Slugification

OpenAPI tags → CLI-friendly slugs:

- Lowercase, spaces to hyphens
- `(Deprecated)` / `(Alpha)` stripped from slug (handled by markers)
- PascalCase split: `DiscoveryIntegration` → `discovery-integration`

Original display name preserved in output. Slugs used only as CLI arguments.

## Documentation Surfacing

OpenAPI specs contain documentation at multiple levels. Phyllotaxis surfaces it based on context:

| Source | Where it appears | Behavior |
|--------|-----------------|----------|
| `info.description` | Level 0 overview | Show first ~200 chars as API description |
| Tag `description` | Level 2 resource detail | Show below resource name |
| Operation `summary` | Level 1-2 listings | Short one-liner next to endpoint |
| Operation `description` | Level 3 detail | Full text below endpoint path (truncate at ~500 chars) |
| Schema `description` | Schema detail view | Below schema name |
| Property `description` | Field listings | Inline with field definition |
| `externalDocs` | Any level where present | "See also:" link at bottom, alongside drill-deeper hints |

**When descriptions are missing:** Omit the field entirely. No "No description" placeholders. Absence communicates sparsity without wasting tokens.

**When descriptions are very long:** Truncate at ~500 chars with `...` to maintain token efficiency. The LLM can follow externalDocs links for full prose.

## Error Handling

- Spec not found: helpful message with resolution order
- Invalid spec: parse error with file path and line number
- Unknown resource/schema: "not found" with closest matches from search
- Unsupported spec version: clear message (openapiv3 supports 3.0.x only)

## Success Criteria

- LLM agent can navigate from API overview to constructing a full API request using only phyllotaxis commands
- Token usage for exploring a specific endpoint is <10% of loading the full spec
- All 31 resource groups, 142 operations, and 143 schemas in the Aembit spec are accessible
- `--json` output is parseable and contains the same information as plain text

## POC Scope

**In scope:**
- All commands: overview, resources (L1-L3), schemas, auth, search, init
- Global flags: `--spec`, `--json`, `--expand`
- Config file with variable overrides
- Deprecation and alpha markers
- Schema composition handling (allOf/oneOf/anyOf)
- Field format and nullable information
- Documentation surfacing (summaries, descriptions, externalDocs)

**Out of scope:**
- User docs integration
- Doc-to-spec association
- Custom content injection
- Fuzzy search (substring only for POC)
- OpenAPI 3.1 support (openapiv3 limitation — revisit post-POC)

## Open Questions

None — all gaps resolved during brainstorm session.

---

## Next Steps

Ready for implementation planning with Plan skill.
