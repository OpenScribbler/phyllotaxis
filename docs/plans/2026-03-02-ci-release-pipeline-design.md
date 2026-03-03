# CI & Release Pipeline - Design Document

**Goal:** Operationalize phyllotaxis as a securely distributable Rust binary with a hardened CI pipeline, cross-platform release builds, SHA256 checksums, and consistent release processes.

**Decision Date:** 2026-03-02

---

## Problem Statement

Phyllotaxis has a basic CI (test, audit, fmt) but no release pipeline, no binary distribution, and no supply chain security hardening. As we prepare to open source, we need:
- Cross-platform binary distribution (5 targets)
- Artifact integrity verification (SHA256 checksums)
- CI hardened against supply chain attacks (pinned action SHAs, restricted permissions)
- Consistent release process with templated notes and a guard hook to prevent accidental releases

## Proposed Solution

A two-workflow GitHub Actions setup: hardened CI for every push/PR, and a tag-triggered release workflow that builds cross-platform binaries, generates checksums, and creates GitHub Releases with attached assets. Supported by `cargo-deny` for license/advisory checks, Dependabot for keeping pinned SHAs current, and a Claude Code release guard hook to enforce process.

## Architecture

### Components

| Component | File | Purpose |
|-----------|------|---------|
| CI workflow | `.github/workflows/ci.yml` | Test, lint, audit, deny — runs on push/PR |
| Release workflow | `.github/workflows/release.yml` | Build, package, checksum, publish — runs on tag push |
| Dependabot config | `.github/dependabot.yml` | Auto-PRs for action SHA and cargo dependency updates |
| cargo-deny config | `deny.toml` | License allowlist, advisory checks, duplicate detection |
| Release notes template | `releases/TEMPLATE.md` | Consistent format for hand-written release notes |
| Versioning policy | `VERSIONING.md` | Semver rules for major/minor/patch decisions |
| Release guard hook | `.claude/hooks/release-guard.py` | Blocks tag creation unless release is prepared |

### Release Flow

```
1. Bump version in Cargo.toml
2. Write release notes: copy releases/TEMPLATE.md → releases/v{version}.md
3. Commit both changes
4. Run release prep (creates .release-pending.yml → unblocks guard hook)
5. git tag v{version} && git push origin v{version}
6. CI: build matrix (5 targets) → package archives → generate checksums
7. CI: create GitHub Release → attach archives + checksums.txt
8. Clean up .release-pending.yml
```

## Key Decisions

| Decision | Choice | Reasoning |
|----------|--------|-----------|
| Build strategy | Native runner matrix | Rust cross-compilation is complex; native runners are free for public repos and most reliable |
| Artifact verification | SHA256 checksums only | Covers integrity verification; GPG/cosign can be added later without changing the pipeline |
| CI security | Pinned SHAs + permissions + Dependabot | Prevents supply chain attacks via mutable tags; Dependabot keeps pins current |
| Rust security tooling | cargo-audit + cargo-deny | cargo-deny is a superset that adds license compliance — critical for open source distribution |
| Release trigger | Git tag push (`v*`) | Standard Rust convention; CI validates tag matches Cargo.toml version |
| Release notes | Template + pre-written, with auto-generated fallback | Hand-written notes serve users better than commit dumps; fallback prevents blocked releases |
| Versioning | Manual bump with documented semver policy | Explicit and simple; no tooling overhead |
| Release guard | Claude Code PreToolUse hook | Prevents accidental tags; same pattern as syllago |
| Archive format | .tar.gz (Linux/macOS), .zip (Windows) | Platform conventions; each contains both `phyllotaxis` and `phyll` binaries |

## Build Matrix

| Target Triple | Runner | Archive Format |
|---------------|--------|----------------|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` |
| `x86_64-apple-darwin` | `macos-13` | `.tar.gz` |
| `aarch64-apple-darwin` | `macos-latest` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` |

## CI Workflow (ci.yml) — Hardened

### Security hardening applied:
- **Pinned commit SHAs** on all `uses:` directives (with human-readable version comments)
- **Permissions blocks** restricting `GITHUB_TOKEN` to read-only (`contents: read`)
- **`--locked` flag** on all cargo commands (ensures Cargo.lock is respected)

### Jobs:
1. **test** — `cargo build --locked && cargo test --locked && cargo clippy --locked -- -D warnings`
2. **fmt** — `cargo fmt --check`
3. **audit** — `cargo audit` (weekly schedule + on push)
4. **deny** — `cargo-deny check` (licenses, advisories, duplicates)

### Pinned Action SHAs (to be verified at implementation time):

```yaml
actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5         # v4
dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9   # stable
Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5       # v2
EmbarkStudios/cargo-deny-action@3fd3802e88374d3fe9159b834c7714ec57d6c979  # v2
actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02   # v4
actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4
```

## Release Workflow (release.yml)

### Trigger
```yaml
on:
  push:
    tags:
      - 'v*'
```

### Permissions
```yaml
permissions:
  contents: write  # Needs to create releases and upload assets
```

### Job 1: `validate`
- Extract version from tag (`${GITHUB_REF_NAME#v}`)
- Read version from `Cargo.toml`
- Fail if they don't match

### Job 2: `build` (matrix, depends on validate)
- Checkout, install Rust toolchain for target
- `cargo build --release --locked --target {target}`
- Package into archive:
  - Linux/macOS: `tar -czf phyllotaxis-{target}.tar.gz -C target/{target}/release phyllotaxis phyll`
  - Windows: zip containing `phyllotaxis.exe` and `phyll.exe`
- Upload archive as GitHub Actions artifact

### Job 3: `release` (depends on all builds)
- Download all build artifacts
- Generate `checksums.txt`: `sha256sum phyllotaxis-*.tar.gz phyllotaxis-*.zip > checksums.txt`
- Check for release notes at `releases/${GITHUB_REF_NAME}.md`
- Create GitHub Release:
  - If notes file exists: use it
  - Otherwise: `--generate-notes` fallback
- Attach all archives + `checksums.txt` as release assets

## Dependabot Configuration

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
```

## cargo-deny Configuration (deny.toml)

```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "BSL-1.0",
    "ISC",
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "allow"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

## Release Notes Template

```markdown
# releases/TEMPLATE.md

# Phyllotaxis vX.Y.Z

## What's New
-

## Bug Fixes
-

## Breaking Changes
- None

## Upgrade Notes
-

---
**Full Changelog:** https://github.com/OpenScribbler/phyllotaxis/compare/vPREV...vX.Y.Z
```

## Versioning Policy (VERSIONING.md)

| Bump | When | Examples |
|------|------|----------|
| **MAJOR** (X.0.0) | Breaking changes to CLI behavior | Remove a command, change flag semantics, drop OpenAPI version support, change default output format |
| **MINOR** (0.X.0) | New capabilities, backwards-compatible | New command, new flags, new OpenAPI features, new output formats |
| **PATCH** (0.0.X) | Fixes only | Bug fixes, performance improvements, dependency updates, output wording fixes |

**Pre-1.0 convention:** Minor versions are treated as additive (no breaking changes in 0.x minors). This builds good habits and avoids surprising early adopters.

## Release Guard Hook

Claude Code `PreToolUse` hook (adapted from syllago). Triggers on `Bash` tool calls that match:
- `git tag v*` (creating version tags)
- `git push ... v*` (pushing version tags)
- `git push --tags` (pushing all tags)

Blocks unless `.release-pending.yml` exists at repo root with `status: prepared`.

The `.release-pending.yml` file is created manually or via a release prep checklist:
```yaml
status: prepared
version: "0.2.0"
date: "2026-03-02"
notes: "releases/v0.2.0.md"
```

After the release is pushed, `.release-pending.yml` is deleted and `.gitignore`d.

## Error Handling

| Failure | Handling |
|---------|----------|
| Tag/Cargo.toml version mismatch | `validate` job fails with clear error message before any builds start |
| Build failure on one target | Matrix job fails; other targets still build. Release job won't run. |
| Missing release notes file | Falls back to GitHub auto-generated notes (not a blocker) |
| cargo-deny finds license issue | CI fails; must resolve before merge |
| cargo audit finds vulnerability | CI fails; must update or justify |

## Success Criteria

- [ ] CI runs on every push/PR with pinned SHAs and permissions blocks
- [ ] cargo-deny checks licenses, advisories, and duplicates
- [ ] Tag push builds binaries for all 5 targets
- [ ] GitHub Release is created with archives and checksums.txt
- [ ] Tag version must match Cargo.toml version
- [ ] Release guard hook prevents accidental tag creation
- [ ] Dependabot auto-PRs for action and cargo dependency updates

## Open Questions

- **Homebrew tap**: Not included in this design. Can be added later as a separate workflow step (like syllago does) once there's demand.
- **Install script**: Deferred. Add if users request it.
- **Cosign signing**: Deferred. SHA256 checksums are sufficient for now; cosign can be layered on later.

---

## Next Steps

Ready for implementation planning with the `Plan` skill.
