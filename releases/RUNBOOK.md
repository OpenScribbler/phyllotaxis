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
