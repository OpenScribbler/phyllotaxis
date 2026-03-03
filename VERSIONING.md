# Versioning Policy

Phyllotaxis follows [Semantic Versioning](https://semver.org/) (MAJOR.MINOR.PATCH).

## When to Bump

| Bump | When | Examples |
|------|------|----------|
| **MAJOR** (X.0.0) | Breaking changes to CLI behavior | Remove a command, change flag semantics, drop OpenAPI version support, change default output format |
| **MINOR** (0.X.0) | New capabilities, backwards-compatible | New command, new flags, new OpenAPI features, new output formats |
| **PATCH** (0.0.X) | Fixes only | Bug fixes, performance improvements, dependency updates, output wording fixes |

## Pre-1.0 Convention

While the project is at 0.x, minor versions are treated as additive (no breaking changes
in 0.x minors). This builds good habits and avoids surprising early adopters.

Breaking changes before 1.0 are still possible, but they will be called out explicitly
in release notes and will bump the minor version with a clear warning.

## Release Process

See `.release-pending.yml` and `releases/TEMPLATE.md` for the release checklist.
Version bumps are made manually in `Cargo.toml` — no tooling automation.
