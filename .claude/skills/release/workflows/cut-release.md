# Cut Release Workflow

> **Trigger:** "release", "tag", "cut a release", "bump version"

## Prerequisites

- On the `main` branch, up to date with origin
- All intended changes are merged
- CI is green on `main`
- Working tree is clean (no uncommitted changes)

## Arguments

The skill accepts one argument: a bump type or explicit version.

| Argument | Effect | Example |
|----------|--------|---------|
| `patch` | Bump patch (0.3.1 -> 0.3.2) | `/release patch` |
| `minor` | Bump minor, reset patch (0.3.1 -> 0.4.0) | `/release minor` |
| `major` | Bump major, reset minor+patch (0.3.1 -> 1.0.0) | `/release major` |
| `X.Y.Z` | Set exact version | `/release 1.0.0` |

If no argument is given, ask the user which bump type to use. Show the current version and what each bump would produce.

## Versioning Rules

From `VERSIONING.md`:

| Bump | When |
|------|------|
| **MAJOR** | Breaking changes to CLI behavior |
| **MINOR** | New capabilities, backwards-compatible |
| **PATCH** | Fixes only (bugs, perf, deps, wording) |

**Pre-1.0 exception:** While at 0.x, minor versions are strictly additive (no breaking changes in 0.x minors). Breaking changes before 1.0 bump the minor version with an explicit callout in release notes.

## Workflow Steps

Execute these steps **sequentially**. Do not skip or reorder. Confirm with the user before proceeding past Step 7 (push to main — the point of no return).

### Step 1: Validate prerequisites

```bash
# Must be on main
git branch --show-current  # expect: main

# Must be clean
git status --porcelain     # expect: empty (or only untracked .release-pending.yml)

# Must be up to date
git fetch origin main
git diff HEAD origin/main --stat  # expect: empty
```

If any check fails, stop and tell the user what needs to be fixed.

### Step 2: Determine new version

Read the current version from `Cargo.toml`:
```bash
grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'
```

Calculate the new version based on the argument. Show the user:
```
Current version: 0.3.1
New version:     0.3.2 (patch bump)
```

### Step 3: Bump version in Cargo.toml

Edit `Cargo.toml` to set the new version. Then regenerate the lockfile:
```bash
cargo check  # updates Cargo.lock without --locked
```

Verify the bump:
```bash
cargo build --locked --bin phyll 2>&1 | tail -3  # should succeed with --locked now
```

### Step 4: Write release notes

Check what changed since the last tag:
```bash
PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -n "$PREV_TAG" ]; then
  git log --oneline ${PREV_TAG}..HEAD
else
  # First release — show all commits
  git log --oneline
fi
```

**For patch releases:** Ask the user if they want to write release notes or let GitHub auto-generate them. Patch releases often don't need custom notes.

**For minor/major releases:** Copy the template and fill it in:
```bash
cp releases/TEMPLATE.md releases/vX.Y.Z.md
```

Edit the release notes file:
- Replace `vX.Y.Z` with the actual version
- Replace `vPREV` in the changelog URL with the previous tag
- Fill in sections based on the commit log
- Present to the user for review/editing

### Step 5: Run local CI gates

All four gates must pass:
```bash
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt --check
cargo deny check
```

If any gate fails, stop and fix the issue before continuing.

### Step 6: Commit

Stage and commit the version bump (and release notes if written):
```bash
git add Cargo.toml Cargo.lock
# If release notes were written:
git add releases/vX.Y.Z.md
git commit -m "chore: release vX.Y.Z"
```

### Step 7: Push to main

```bash
git push origin main
```

**Important:** At this point the commit is public. If something goes wrong after this, we'll need to fix forward, not revert.

### Step 8: Wait for CI on main

Check that CI passes on the pushed commit before tagging:
```bash
gh run list --branch main --limit 1
```

If CI is failing, fix the issue before proceeding. Do not tag a broken commit.

### Step 9: Create the release sentinel

The release guard hook (`.claude/hooks/release-guard.py`) blocks tag creation unless this file exists:

```bash
cat > .release-pending.yml << 'EOF'
status: prepared
version: "X.Y.Z"
date: "YYYY-MM-DD"
notes: "releases/vX.Y.Z.md"
EOF
```

Replace the placeholders with actual values. The `notes` field can be omitted if no custom release notes were written.

### Step 10: Tag and push

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

**Do NOT use `git push --tags`** — that pushes ALL local tags, which can cause noise from old/stale tags.

### Step 11: Clean up

```bash
rm .release-pending.yml
```

### Step 12: Monitor

Tell the user to watch the GitHub Actions Release workflow:
- `validate` — confirms tag matches Cargo.toml
- `build` — 5-target matrix (linux x86/arm, macOS x86/arm, windows)
- `release` — creates GitHub Release with archives + checksums
- `publish-crate` — publishes to crates.io
- `update-homebrew` — updates the Homebrew tap formula

Provide the Actions URL:
```
https://github.com/OpenScribbler/phyllotaxis/actions
```

## If Something Goes Wrong

**validate fails (tag/Cargo.toml mismatch):**
```bash
# Delete the tag and re-tag after fixing
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
# Fix, commit, re-create sentinel, re-tag, re-push
```

**A build target fails:**
The release job won't run. Fix the build, delete the tag, re-push.

**Release job fails (transient):**
Re-run from the Actions UI. If partially created:
```bash
gh release delete vX.Y.Z --yes
```
Then re-run.
