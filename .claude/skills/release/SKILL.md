---
name: release
description: Cut a versioned release of phyllotaxis. USE WHEN releasing OR tagging OR bumping version OR cutting a release. Handles version bump, release notes, CI gates, sentinel, tagging, and push.
---

# release

Guided release workflow for phyllotaxis. Ensures every step is completed in order — version bump, release notes, local CI checks, commit, sentinel creation, tag, push, and cleanup.

## Workflow Routing

| Workflow | Trigger | File |
|----------|---------|------|
| **cut-release** | "release", "tag", "cut a release", "bump version" | `workflows/cut-release.md` |

## Examples

**Example 1: Patch release after a bug fix**
```
User: "/release patch"
-> Determines next version (0.3.1 -> 0.3.2)
-> Bumps Cargo.toml, regenerates Cargo.lock
-> Prompts for release notes (or skips for auto-generated)
-> Runs local CI gates
-> Commits, creates sentinel, tags, pushes
-> Cleans up sentinel
```

**Example 2: Minor release with new features**
```
User: "/release minor"
-> Determines next version (0.3.1 -> 0.4.0)
-> Bumps Cargo.toml, regenerates Cargo.lock
-> Creates release notes from template
-> Runs local CI gates
-> Commits, creates sentinel, tags, pushes
-> Cleans up sentinel
```

**Example 3: Release with explicit version**
```
User: "/release 1.0.0"
-> Sets version to 1.0.0
-> Full release flow
```
