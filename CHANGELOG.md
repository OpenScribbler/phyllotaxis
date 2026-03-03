# Changelog

All notable changes to this project will be documented in this file.

This project follows [Semantic Versioning](https://semver.org/). See [VERSIONING.md](VERSIONING.md) for the full policy.

## [0.2.0] - 2026-03-03

First public release.

### New Features
- **Example generation** (`--example`) — auto-generate JSON examples from any schema, using spec-provided examples or type-based placeholders
- **Reverse schema lookup** (`--used-by`) — find which endpoints use a given schema (request body, response, or nested)
- **Related schemas** (`--context`) — expand referenced schemas inline when viewing endpoint detail
- **Search expansion** — search now covers parameter names, descriptions, request body descriptions, and response descriptions
- **Shell completions** (`completions` command) — bash, zsh, fish, powershell, elvish
- **Fuzzy matching** — mistyped resource/schema names suggest close matches
- **Helpful error messages** — detects common mistakes like quoting "GET /path" as one argument
- **CI pipeline** — test, clippy, fmt, audit, deny gates on every push
- **Release pipeline** — automated multi-platform binary builds (Linux x86/ARM, macOS Intel/Apple Silicon, Windows)

### Bug Fixes
- Fix duplicate error messages (errors no longer print twice)
- Fix inline request body schemas (non-`$ref` bodies now resolve correctly)
- Fix `anyOf`/`oneOf` schema compositions that rendered as empty variants
- Fix inline list/pagination wrapper response schemas
- Fix `--expand` not inlining nested objects in request bodies
- Fix array-of-`anyOf`/`oneOf` schema compositions
- Include "Did you mean?" suggestions in JSON error output
- Resolve 9 audit findings for security and correctness

## [0.1.0] - 2026-03-02

Initial development release (not published).

### Features
- Progressive disclosure: overview, resources, resource detail, endpoint detail
- Schema listing and detail with field types, composition, and expansion
- Auth scheme extraction
- Cross-type search
- Webhook callback extraction
- Spec discovery (flag, env var, config file, auto-detect)
- Multi-spec project support via `.phyllotaxis.yaml`
- JSON output mode (`--json`) for every command
- `phyll init` for interactive setup with framework detection
