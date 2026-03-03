# CI & Release Pipeline - Implementation Plan

**Feature:** ci-release-pipeline
**Design Doc:** `docs/plans/2026-03-02-ci-release-pipeline-design.md`
**Date:** 2026-03-02

---

## Overview

This plan implements the full CI & release pipeline in five groups. Each task is sized for
a 2-5 minute human+AI pairing session. Work them in order — later groups depend on earlier
ones.

**Groups:**
1. CI Hardening (tasks 1-3)
2. cargo-deny Setup (tasks 4-5)
3. Dependabot (task 6)
4. Release Workflow (tasks 7-8)
5. Release Process Infrastructure (tasks 9-13)

---

## Group 1: CI Hardening

### Task 1: Replace tag references with pinned SHAs in ci.yml

**Files to modify:** `.github/workflows/ci.yml`

**What and why:** Mutable tags like `@v4` are a supply chain risk — an attacker who compromises
the upstream action repo can silently change what code runs in CI. Pinning to a commit SHA means
CI always runs exactly the same action code unless we explicitly update the pin.

**Full updated file content:**

```yaml
name: CI

on:
  push:
  pull_request:
  schedule:
    - cron: '0 8 * * 1'

permissions:
  contents: read

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    if: github.event_name != 'schedule'
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9 # stable
      - uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2
      - run: cargo build --locked
      - run: cargo test --locked
      - run: cargo clippy --locked -- -D warnings

  fmt:
    name: Format
    runs-on: ubuntu-latest
    if: github.event_name != 'schedule'
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9 # stable
        with:
          components: rustfmt
      - run: cargo fmt --check

  audit:
    name: Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9 # stable
      - run: cargo install cargo-audit --locked
      - run: cargo audit

  deny:
    name: Deny
    runs-on: ubuntu-latest
    if: github.event_name != 'schedule'
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: EmbarkStudios/cargo-deny-action@3fd3802e88374d3fe9159b834c7714ec57d6c979 # v2
```

**Changes from current ci.yml:**
- All three `actions/checkout@v4` → pinned SHA
- All three `dtolnay/rust-toolchain@stable` → pinned SHA
- Both `Swatinem/rust-cache@v2` → pinned SHA
- Added top-level `permissions: contents: read` block
- Added `deny` job using `EmbarkStudios/cargo-deny-action` pinned SHA
- `deny` job skips on schedule (only needed on code changes)

**Verify:** Push a commit. In Actions, all job steps should show commit SHAs (not tag names)
in the workflow summary. `cargo-deny check` will fail until Task 4 creates `deny.toml` —
that's expected and fine. Complete Task 4 before merging Task 1.

---

### Task 2: Verify pinned SHAs are current (informational checkpoint)

**Files to modify:** None — this is a verification step before committing.

**What to do:** The SHAs in the design doc were specified on 2026-03-02. Confirm they still
resolve to the intended versions using GitHub's commit API or by checking the action repos.
If any SHA has changed (the action was updated), update to the new SHA of the same version.

Check commands (run locally or in browser):
```
https://github.com/actions/checkout/commit/34e114876b0b11c390a56381ad16ebd13914f8d5
https://github.com/dtolnay/rust-toolchain/commit/efa25f7f19611383d5b0ccf2d1c8914531636bf9
https://github.com/Swatinem/rust-cache/commit/779680da715d629ac1d338a641029a2f4372abb5
https://github.com/EmbarkStudios/cargo-deny-action/commit/3fd3802e88374d3fe9159b834c7714ec57d6c979
```

**Verify:** Each URL should resolve to a commit tagged as the expected version (v4, stable, v2, v2).

---

### Task 3: Remove schedule guard from `deny` job in ci.yml

**Files to modify:** `.github/workflows/ci.yml`

**What and why:** The `audit` job runs on the weekly schedule to catch new advisories even
without code changes. The `deny` job should do the same — a new advisory or license issue
can appear any day, not just when code changes. Task 1 added `if: github.event_name != 'schedule'`
to `deny` as a conservative starting point; this task removes that guard so `deny` runs on
schedule alongside `audit`.

**Change:** Remove the `if:` condition from the `deny` job:

```yaml
  deny:
    name: Deny
    runs-on: ubuntu-latest
    # No 'if' condition — runs on all events including schedule
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: EmbarkStudios/cargo-deny-action@3fd3802e88374d3fe9159b834c7714ec57d6c979 # v2
```

**Verify:** After the weekly schedule runs, check that both `audit` and `deny` jobs appear
in the workflow run triggered by the cron event.

---

## Group 2: cargo-deny Setup

### Task 4: Create deny.toml

**Files to create:** `deny.toml` (repo root)

**What and why:** `cargo-deny` needs a config file specifying which licenses are acceptable,
what to do about duplicate versions, and where to source advisories from. Without it, the
`deny` CI job will fail or use defaults that may block legitimate dependencies.

**Full file content:**

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

**License reasoning:**
- `MIT`, `Apache-2.0`, `ISC` — standard permissive licenses in the Rust ecosystem
- `Unicode-3.0`, `Unicode-DFS-2016` — required by `unicode-ident` (used by `syn`/`proc-macro2`)
- `BSL-1.0` — used by some Rust crates (e.g. `indexmap`'s dependencies)

**Verify:** Run `cargo deny check` locally. It should pass without errors. If it reports
unknown licenses for any dependency, add those licenses to the `allow` list (after confirming
they are acceptable). Warnings about duplicate versions are expected and OK.

---

### Task 5: Run cargo deny locally and fix any issues before CI

**Files to modify:** `deny.toml` if needed

**What to do:**

```bash
cargo install cargo-deny --locked
cargo deny check
```

Read the output carefully:
- **License errors:** Add the offending license to `deny.toml`'s `allow` list if it's acceptable,
  or file an issue if it's a license that should not be distributed.
- **Advisory errors:** These mean a dependency has a known security advisory. Either update
  the dependency (`cargo update <crate>`) or add an `ignore` entry in `[advisories]` with
  justification if updating isn't possible.
- **Duplicate version warnings:** These are expected (e.g., two crates depending on different
  versions of `syn`). No action needed — they're `warn`, not `deny`.

**Verify:** `cargo deny check` exits with code 0. CI `deny` job passes on the next push.

---

## Group 3: Dependabot

### Task 6: Create .github/dependabot.yml

**Files to create:** `.github/dependabot.yml`

**What and why:** Dependabot automatically opens PRs when GitHub Actions SHAs (which we just
pinned) or Cargo dependencies are updated. Without it, pinned SHAs drift and become stale
security pins that no one updates. With it, SHA updates arrive as reviewable PRs.

**Full file content:**

```yaml
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

**Verify:** After pushing, go to GitHub → Insights → Dependency graph → Dependabot. Verify
Dependabot is enabled for both `github-actions` and `cargo` ecosystems. PRs will appear within
a week (or sooner if Dependabot runs immediately after the config is detected).

---

## Group 4: Release Workflow

### Task 7: Create .github/workflows/release.yml

**Files to create:** `.github/workflows/release.yml`

**Prerequisites:** `Cargo.lock` must be committed. Confirm with `git status Cargo.lock` — if it
is untracked or gitignored, add it and commit before creating this workflow. The `--locked` flag
in `cargo build` will fail if `Cargo.lock` is absent from the repo.

**Note on `ubuntu-24.04-arm`:** This runner label was introduced by GitHub in late 2024 for
ARM-native Linux builds. Verify the repo has access to it — it is available on free-tier public
repos but may require specific runner settings. If not available, the fallback is cross-compilation
using `cross` (requires Docker) or `cargo-zigbuild`. Native is strongly preferred.

**What and why:** This is the core of the release pipeline. Three jobs run in sequence:
1. `validate` — ensures tag version matches `Cargo.toml` before wasting build time
2. `build` — matrix of 5 targets building native binaries in parallel
3. `release` — collects all artifacts, generates checksums, creates GitHub Release

Using a matrix for builds means all 5 targets build in parallel rather than sequentially,
cutting total release time roughly 5x. The `validate` job runs first and gates the matrix —
no builds start if the version tag is wrong.

**Full file content:**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  validate:
    name: Validate tag
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4

      - name: Check tag matches Cargo.toml version
        run: |
          TAG_VERSION="${GITHUB_REF_NAME#v}"
          CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
          echo "Tag version:   $TAG_VERSION"
          echo "Cargo version: $CARGO_VERSION"
          if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
            echo "ERROR: Tag version ($TAG_VERSION) does not match Cargo.toml version ($CARGO_VERSION)"
            exit 1
          fi

  build:
    name: Build - ${{ matrix.target }}
    needs: validate
    runs-on: ${{ matrix.runner }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            runner: ubuntu-latest
            archive: phyllotaxis-x86_64-unknown-linux-gnu.tar.gz
            archive_cmd: tar
          - target: aarch64-unknown-linux-gnu
            runner: ubuntu-24.04-arm
            archive: phyllotaxis-aarch64-unknown-linux-gnu.tar.gz
            archive_cmd: tar
          - target: x86_64-apple-darwin
            runner: macos-13
            archive: phyllotaxis-x86_64-apple-darwin.tar.gz
            archive_cmd: tar
          - target: aarch64-apple-darwin
            runner: macos-latest
            archive: phyllotaxis-aarch64-apple-darwin.tar.gz
            archive_cmd: tar
          - target: x86_64-pc-windows-msvc
            runner: windows-latest
            archive: phyllotaxis-x86_64-pc-windows-msvc.zip
            archive_cmd: zip

    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4

      - uses: dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9 # stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2
        with:
          key: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --locked --target ${{ matrix.target }}

      - name: Package (Linux/macOS)
        if: matrix.archive_cmd == 'tar'
        run: |
          tar -czf ${{ matrix.archive }} \
            -C target/${{ matrix.target }}/release \
            phyllotaxis phyll

      - name: Package (Windows)
        if: matrix.archive_cmd == 'zip'
        shell: pwsh
        run: |
          Compress-Archive -Path `
            "target/${{ matrix.target }}/release/phyllotaxis.exe", `
            "target/${{ matrix.target }}/release/phyll.exe" `
            -DestinationPath "${{ matrix.archive }}"

      - uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4
        with:
          name: ${{ matrix.target }}
          path: ${{ matrix.archive }}

  release:
    name: Create release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4

      - uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4
        with:
          merge-multiple: true

      - name: Generate checksums
        run: |
          sha256sum phyllotaxis-*.tar.gz phyllotaxis-*.zip > checksums.txt
          cat checksums.txt

      - name: Prepare release notes
        id: notes
        run: |
          NOTES_FILE="releases/${GITHUB_REF_NAME}.md"
          if [ -f "$NOTES_FILE" ]; then
            echo "notes_file=$NOTES_FILE" >> $GITHUB_OUTPUT
          fi

      - name: Create GitHub Release (with release notes)
        if: steps.notes.outputs.notes_file != ''
        env:
          GH_TOKEN: ${{ github.token }}
          NOTES_FILE: ${{ steps.notes.outputs.notes_file }}
        run: |
          gh release create "$GITHUB_REF_NAME" \
            --title "$GITHUB_REF_NAME" \
            --notes-file "$NOTES_FILE" \
            phyllotaxis-*.tar.gz phyllotaxis-*.zip \
            checksums.txt

      - name: Create GitHub Release (auto-generated notes)
        if: steps.notes.outputs.notes_file == ''
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          gh release create "$GITHUB_REF_NAME" \
            --title "$GITHUB_REF_NAME" \
            --generate-notes \
            phyllotaxis-*.tar.gz phyllotaxis-*.zip \
            checksums.txt
```

**Verify:** Create a test tag against a throwaway commit to exercise the pipeline. See Task 13
for how to do a real release. For a dry run:
```bash
git tag v0.0.0-test
git push origin v0.0.0-test
```
Watch all three jobs complete in sequence in GitHub Actions. Delete the test tag and release
afterward:
```bash
git tag -d v0.0.0-test
git push origin :refs/tags/v0.0.0-test
# Delete the GitHub Release manually in the UI or via:
gh release delete v0.0.0-test --yes
```

---

### Task 8: Verify Windows packaging works (cross-platform gotcha)

**Files to modify:** `.github/workflows/release.yml` if needed

**What and why:** The Windows `Compress-Archive` PowerShell cmdlet requires that source files
exist and that the path separators are correct. Rust on Windows builds binaries to
`target\x86_64-pc-windows-msvc\release\phyllotaxis.exe`. The workflow uses forward slashes
in the path, which PowerShell handles correctly — but this is worth confirming.

**Potential issue:** If the build step runs in a `cmd` context rather than PowerShell, the
forward slashes in `target/${{ matrix.target }}/release/phyllotaxis.exe` may fail. The
`shell: pwsh` on the Package step handles this correctly.

**Verify:** After the first release run (real or test from Task 7), confirm that
`phyllotaxis-x86_64-pc-windows-msvc.zip` appears as a release asset. Download it and verify
it contains both `phyllotaxis.exe` and `phyll.exe`.

---

## Group 5: Release Process Infrastructure

### Task 9: Create .claude/hooks/release-guard.py

**Files to create:** `.claude/hooks/release-guard.py`

**What and why:** This Claude Code `PreToolUse` hook blocks `git tag v*` and `git push ... v*`
commands unless `.release-pending.yml` exists with `status: prepared`. It prevents accidental
tag creation when you're in the middle of a coding session and type a tag command by reflex.
The hook is taken directly from syllago with no changes — the logic is identical.

**Note:** The `.claude/` directory does not exist yet in phyllotaxis. Create it (and the
`hooks/` subdirectory) as part of this task. Also confirm whether a project-level
`settings.json` already exists at `.claude/settings.json` before overwriting — as of
2026-03-02 it does not exist. The global Claude Code settings at `~/.claude/` are separate
and must not be modified.

**Full file content:**

```python
#!/usr/bin/env python3
"""Release guard hook for Claude Code.

PreToolUse hook that blocks git tag creation and version tag pushing
unless a .release-pending.yml file exists with status: prepared.

This prevents accidental releases — tags can only be created/pushed
after release prep has been completed through the full release flow.
"""

import json
import os
import re
import subprocess
import sys


def get_repo_root():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except Exception:
        return None


def check_release_file(repo_root):
    """Check that .release-pending.yml exists and has status: prepared."""
    release_file = os.path.join(repo_root, ".release-pending.yml")

    if not os.path.exists(release_file):
        print("BLOCKED: Cannot create or push version tags without a prepared release.")
        print("Create .release-pending.yml with status: prepared to proceed.")
        sys.exit(1)

    with open(release_file) as f:
        content = f.read()

    if "status: prepared" not in content:
        print("BLOCKED: Release is not in 'prepared' state.")
        for line in content.splitlines():
            if line.startswith("status:"):
                print(f"Current state: {line.strip()}")
        sys.exit(1)


def main():
    stdin_data = sys.stdin.read().strip()
    if not stdin_data:
        sys.exit(0)

    try:
        payload = json.loads(stdin_data)
    except json.JSONDecodeError:
        sys.exit(0)

    if payload.get("tool_name") != "Bash":
        sys.exit(0)

    cmd = payload.get("tool_input", {}).get("command", "")

    # Detect git tag creation (exclude listing: -l, --list)
    creates_tag = bool(re.search(r"git\s+tag\s+(?!-l\b|--list\b)", cmd))

    # Detect pushing version tags (v0.x.x patterns or --tags flag)
    pushes_version_tag = bool(re.search(r"git\s+push.*\bv\d", cmd))
    pushes_all_tags = bool(re.search(r"git\s+push\s+--tags", cmd))

    if not creates_tag and not pushes_version_tag and not pushes_all_tags:
        sys.exit(0)

    repo_root = get_repo_root()
    if not repo_root:
        sys.exit(0)

    check_release_file(repo_root)

    # All checks passed — allow the command
    sys.exit(0)


if __name__ == "__main__":
    main()
```

**After creating the file, make it executable:**

```bash
chmod +x /home/hhewett/.local/src/phyllotaxis/.claude/hooks/release-guard.py
```

**Then register it in `.claude/settings.json`** (create the file if it doesn't exist):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 .claude/hooks/release-guard.py"
          }
        ]
      }
    ]
  }
}
```

**Verify:** From the phyllotaxis repo root in a Claude Code session, ask Claude to run:
```bash
git tag v99.99.99
```
It should be blocked with the "BLOCKED: Cannot create or push version tags without a prepared
release." message. Then create `.release-pending.yml` with `status: prepared` and confirm the
same command is now allowed (don't actually push it — just verify the hook passes).

---

### Task 10: Create releases/TEMPLATE.md

**Files to create:** `releases/TEMPLATE.md`

**What and why:** Every release needs notes. A template enforces a consistent structure and
makes it easy to write release notes during the release process. The `releases/` directory
is where per-release notes live (e.g., `releases/v0.2.0.md`), and the release workflow
checks for them by tag name.

**Full file content:**

```markdown
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

**Verify:** File exists at `releases/TEMPLATE.md`. It will be used as the starting point
every time a release is prepared (copy it to `releases/v{version}.md` and fill it in).

---

### Task 11: Create VERSIONING.md

**Files to create:** `VERSIONING.md` (repo root)

**What and why:** Documents the semver policy for contributors and anyone deciding whether
a change warrants a major/minor/patch bump. At pre-1.0, this also clarifies that 0.x minors
are treated as non-breaking.

**Full file content:**

```markdown
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
```

**Verify:** File exists at `VERSIONING.md`. No functional verification needed.

---

### Task 12: Add .release-pending.yml to .gitignore

**Files to modify:** `.gitignore`

**What and why:** `.release-pending.yml` is a transient local file used to signal the
release guard hook. It should never be committed — committing it would permanently unblock
the guard for all future sessions, defeating the purpose.

**Change:** Add to `.gitignore`:

```
# Release guard sentinel (do not commit)
.release-pending.yml
```

**Full updated .gitignore:**

```
/target

# External API specs downloaded for manual testing (large files)
tests/fixtures/github.yaml
tests/fixtures/stripe.yaml
tests/fixtures/slack.yaml
tests/fixtures/gitlab.yaml
tests/fixtures/digitalocean.*
tests/fixtures/spotify.*
tests/fixtures/twilio.yaml

# Local config
.phyllotaxis.yaml

# Release guard sentinel (do not commit)
.release-pending.yml
```

**Verify:** Create a `.release-pending.yml` at repo root, then run `git status`. It should
not appear as an untracked file.

---

### Task 13: Document the release process (release runbook)

**Files to create:** `releases/RUNBOOK.md`

**What and why:** The release guard hook, TEMPLATE.md, and workflow are only useful if the
release process is written down. This runbook is the authoritative step-by-step guide for
cutting a release.

**Full file content:**

```markdown
# Release Runbook

How to cut a release of phyllotaxis.

## Prerequisites

- You are on the `main` branch, up to date with origin.
- All intended changes are merged.
- CI is green.

## Steps

### 1. Bump the version

Edit `Cargo.toml`:
```toml
version = "0.2.0"   # was 0.1.0
```

Refer to `VERSIONING.md` for which number to bump.

### 2. Write release notes

Copy the template and fill it in:

```bash
cp releases/TEMPLATE.md releases/v0.2.0.md
```

Edit `releases/v0.2.0.md`:
- Replace `vX.Y.Z` with the actual version
- Replace `vPREV` in the changelog URL with the previous tag (e.g., `v0.1.0`)
- Fill in What's New, Bug Fixes, Breaking Changes, Upgrade Notes

### 3. Commit both changes

```bash
git add Cargo.toml releases/v0.2.0.md
git commit -m "chore: release v0.2.0"
git push origin main
```

Wait for CI to pass on `main`.

### 4. Create the release sentinel

Create `.release-pending.yml` at the repo root (this file is gitignored):

```yaml
status: prepared
version: "0.2.0"
date: "2026-03-02"
notes: "releases/v0.2.0.md"
```

### 5. Tag and push

The release guard hook will check for the sentinel before allowing these commands:

```bash
git tag v0.2.0
git push origin v0.2.0
```

### 6. Monitor the release

Go to GitHub Actions and watch the Release workflow:
- `validate` job should pass (confirms tag matches Cargo.toml)
- `build` job matrix (5 targets) should all succeed
- `release` job creates the GitHub Release with archives and checksums.txt

The full pipeline typically takes 5-10 minutes.

### 7. Clean up

Delete the sentinel:

```bash
rm .release-pending.yml
```

Verify the GitHub Release looks correct — release notes, all 5 archives, checksums.txt.

## If Something Goes Wrong

**validate fails:** Tag and Cargo.toml versions don't match. Delete the tag, bump Cargo.toml
or fix the tag, and retry.
```bash
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
# Fix the mismatch, then re-tag
```

**One build target fails:** The release job won't run. Fix the build issue, delete the tag,
and re-push it. The partial artifacts from the failed run will be cleaned up automatically.

**Release job fails:** Check the logs. If it's a transient GitHub API error, re-run the job
from the Actions UI. If the release was partially created, delete it before re-running:
```bash
gh release delete v0.2.0 --yes
```
```

**Verify:** File exists at `releases/RUNBOOK.md`. Read through it once to confirm every
step matches the implementation (particularly the sentinel file format, which must match
what release-guard.py checks).

---

## Implementation Order and Dependencies

```
Task 2 (verify SHAs)
  └── Task 1 (update ci.yml)
        └── Task 4 (create deny.toml)
              └── Task 5 (run deny locally)
                    └── Task 3 (schedule condition decision)

Task 6 (dependabot.yml)   [independent, do anytime after Task 1]

Task 7 (release.yml)
  └── Task 8 (verify Windows packaging)   [requires a test run]

Task 9 (release-guard.py)
Task 10 (releases/TEMPLATE.md)
Task 11 (VERSIONING.md)
Task 12 (.gitignore update)
Task 13 (releases/RUNBOOK.md)
  [Tasks 9-13 are independent of each other, do in any order]
```

**Recommended commit grouping:**
- Commit 1: Tasks 1 + 4 + 5 together (ci.yml + deny.toml, verified locally)
- Commit 2: Task 6 (dependabot.yml)
- Commit 3: Task 7 (release.yml)
- Commit 4: Tasks 9 + 10 + 11 + 12 + 13 together (all release process files)

---

## Files Created/Modified Summary

| File | Action | Task |
|------|--------|------|
| `.github/workflows/ci.yml` | Modify — pin SHAs, add permissions, add deny job | 1, 3 |
| `deny.toml` | Create | 4 |
| `.github/dependabot.yml` | Create | 6 |
| `.github/workflows/release.yml` | Create | 7 |
| `.claude/hooks/release-guard.py` | Create | 9 |
| `.claude/settings.json` | Create | 9 |
| `releases/TEMPLATE.md` | Create | 10 |
| `VERSIONING.md` | Create | 11 |
| `.gitignore` | Modify — add .release-pending.yml | 12 |
| `releases/RUNBOOK.md` | Create | 13 |
