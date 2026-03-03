# Phase B Analysis: ci-release-pipeline

Generated: 2026-03-02
Tasks analyzed: 13

---

## Task 1: Replace tag references with pinned SHAs in ci.yml

- [x] Implicit deps: Depends on Task 2 completing verification (already captured in existing deps). The plan provides a complete file replacement — no partial diffs — so no prior state needs to be understood.
- [x] Missing context: The current ci.yml has no `fmt` job listed in Task 1's replacement content. Cross-checking: current ci.yml has `test`, `audit`, `fmt`. Task 1's replacement YAML includes all four (`test`, `fmt`, `audit`, `deny`). The plan's "Changes from current ci.yml" section doesn't mention `fmt` because it was already there — but this should be explicit. The file replacement approach (full content provided) means this can't be missed in practice.
- [x] Hidden blockers: None. All SHAs are hardcoded in the plan. GitHub Actions doesn't require any repo settings to use third-party actions.
- [x] Cross-task conflicts: Task 3 also modifies ci.yml (removes `if:` guard from `deny` job). These cannot be done in the same commit safely — Task 3 must apply after Task 1. The existing dep (Task 3 → Task 1) handles this.
- [x] Success criteria: Push a commit. In GitHub Actions, each `uses:` step in the workflow summary shows a full 40-character SHA (not a tag like `v4`). The `deny` job appears but fails with "could not find `deny.toml`" or similar — that is the expected partial failure state before Task 4.

**Actions taken:**
- None required. Existing Task 1 → Task 2 and Task 3 → Task 1 dependencies already handle ordering.

---

## Task 2: Verify pinned SHAs are current (informational checkpoint)

- [x] Implicit deps: None. This is a pre-flight check with no file modifications.
- [x] Missing context: The task instructs verifying that each GitHub URL resolves to a commit tagged as the expected version. It does NOT specify what to do if the SHA is stale. If a SHA points to an old commit (the action was re-tagged to a newer SHA), the agent needs to know: look up the tag's current SHA on GitHub and substitute it in the full Task 1 YAML. This substitution step is implied but not stated. Low risk in practice — SHA staleness for these stable, rarely-changed actions is unlikely over days.
- [x] Hidden blockers: Requires browser or `curl`/`gh api` access to GitHub. In a headless agent context, this is `gh api repos/actions/checkout/commits/34e114876b0b11c390a56381ad16ebd13914f8d5`. If the agent lacks `gh` auth, this step is blocked.
- [x] Cross-task conflicts: None (no file modifications).
- [x] Success criteria: All four GitHub commit URLs return HTTP 200 with a commit page that shows the expected version tag (`v4`, `stable`, `v2`, `v2`) in the commit metadata. If any returns 404 or the tag is different, the correct current SHA is looked up and the Task 1 file content is updated before proceeding.

**Actions taken:**
- None required. Risk is low given action repo stability.

---

## Task 3: Remove schedule guard from `deny` job in ci.yml

- [x] Implicit deps: Depends on Task 1 (already captured). Also implicitly depends on Task 4 (`deny.toml`) being committed — without it, the `deny` job will fail on every run including the weekly schedule, producing noise. The plan acknowledges this in Task 1's verify section ("will fail until Task 4 creates deny.toml") but doesn't model it as a dependency here. Task 3 should not be merged to main until Task 4 is also merged.
- [x] Missing context: The task shows a partial YAML snippet, not a full file replacement. The agent needs to know: edit the existing ci.yml (which by this point already has Task 1's content applied) and remove the `if: github.event_name != 'schedule'` line from the `deny` job only. This is unambiguous.
- [x] Hidden blockers: None beyond the `deny.toml` readiness noted above.
- [x] Cross-task conflicts: Modifies ci.yml. Task 1 also modifies ci.yml. Sequencing is enforced by the existing dep. No conflict if applied in order.
- [x] Success criteria: ci.yml `deny` job has no `if:` condition. After the next Monday 08:00 UTC cron trigger, both `audit` and `deny` jobs appear in the resulting workflow run in GitHub Actions.

**Actions taken:**
- Added hidden dependency note: Task 3 should not be merged independently of Task 4. No bead dep added (the plan already recommends Commit 1 bundles Tasks 1 + 4 + 5 together, which naturally handles this).

---

## Task 4: Create deny.toml

- [x] Implicit deps: Depends on Task 1 (already captured — the `deny` job in ci.yml must exist before deny.toml has any effect). Also implicitly depends on Task 5 completing cleanly: the plan says "Complete Task 4 before merging Task 1," but Task 4 is only verified by running Task 5. These two should be treated as a unit.
- [x] Missing context: The license list may be incomplete. Current dependencies include `indexmap 2.13.0`, `serde`, `clap`, `anyhow`, `openapiv3`, `serde_yaml_ng`, `strsim`, `human-panic`, `clap_complete`, `tempfile`. The plan notes `BSL-1.0` for indexmap dependencies — this should be verified, as indexmap itself is MIT/Apache-2.0. The real `BSL-1.0` user in common Rust ecosystems is `hashbrown` (MIT/Apache-2.0 in recent versions). Task 5 is the correct place to discover any missing licenses, so the incompleteness is intentional and handled.
- [x] Hidden blockers: `cargo-deny` must be installed locally to run Task 5. It is NOT currently installed (`cargo install --list` shows neither `cargo-deny` nor `cargo-audit`). Task 5 includes `cargo install cargo-deny --locked` which handles this, but the install itself takes 1-3 minutes and requires a working Rust toolchain and internet access.
- [x] Cross-task conflicts: New file, no conflicts.
- [x] Success criteria: `deny.toml` exists at repo root. `cargo deny check` exits with code 0 (after any license additions discovered in Task 5). No license errors; duplicate version warnings are acceptable.

**Actions taken:**
- None required. Task 5 already accounts for the discovery loop.

---

## Task 5: Run cargo deny locally and fix any issues before CI

- [x] Implicit deps: Depends on Task 4 (`deny.toml` must exist). Already captured.
- [x] Missing context: `cargo-deny` is not installed locally. The install step `cargo install cargo-deny --locked` is included — this is correct. However, `cargo-deny` requires network access to clone the advisory database on first run (`db-urls`). If running in an offline or restricted network environment, the first `cargo deny check` will fail on advisory fetching even if licenses are fine. The plan does not mention this. Low risk in practice but worth noting.
- [x] Hidden blockers: Network access required for advisory DB download on first run. Already noted above.
- [x] Cross-task conflicts: May modify `deny.toml`. Task 4 creates it. Sequential, no conflict.
- [x] Success criteria: `cargo deny check` exits with code 0 with the final `deny.toml`. No error-level output (warnings for duplicate versions are fine). The exact version of `cargo-deny` installed should be noted so it matches CI behavior (CI uses `EmbarkStudios/cargo-deny-action@v2` which bundles its own version).

**Actions taken:**
- None required. Risk is informational only.

---

## Task 6: Create .github/dependabot.yml

- [x] Implicit deps: The plan lists this as independent ("do anytime after Task 1"). However, Dependabot for `github-actions` ecosystem will immediately try to update the pinned SHAs added in Task 1. If Task 1 is not yet merged, Dependabot has nothing to update and will produce no PRs. Functionally independent for file creation, but logically dependent on Task 1's SHAs being in the repo for Dependabot to do anything useful. A bead dependency was added: Task 6 → Task 1.
- [x] Missing context: The config does not specify a `target-branch` for Dependabot PRs. By default Dependabot targets the repo's default branch (`main`). This is correct behavior and no change is needed.
- [x] Hidden blockers: Dependabot must be enabled for the repository in GitHub Settings → Security → Dependabot. For public repos it is typically auto-enabled. The verify step correctly directs to check the Insights → Dependency graph → Dependabot page.
- [x] Cross-task conflicts: New file. No conflicts.
- [x] Success criteria: `.github/dependabot.yml` exists. GitHub Insights → Dependency graph → Dependabot shows two monitored ecosystems: `github-actions` and `cargo`. At least one Dependabot PR (or "up to date" status) appears within 24 hours of pushing.

**Actions taken:**
- Added bead dependency: `phyllotaxis-iuc` (Task 6) depends on `phyllotaxis-5hk` (Task 1).

---

## Task 7: Create .github/workflows/release.yml

- [x] Implicit deps: None stated beyond sha verification from Task 2 (same SHAs are reused in release.yml). Also uses `actions/upload-artifact` and `actions/download-artifact` SHAs that appear in the plan but were only listed in the design doc — they are included in the Task 7 YAML content so no lookup is needed. Critical implicit dep: `Cargo.lock` must be committed (it is — `git ls-files Cargo.lock` confirms). The `--locked` build flag will fail without it.
- [x] Missing context: Two issues added to the plan as a direct edit:
  1. `ubuntu-24.04-arm` runner availability — this label is relatively new and requires verification that the repo has access. For public repos on GitHub free tier it should be available, but if this is a private repo or the label has changed, the ARM Linux build will queue indefinitely. The plan now documents this with a fallback note.
  2. The `Cargo.lock` requirement is now documented explicitly in the plan.
- [x] Hidden blockers: `ubuntu-24.04-arm` runner availability (see above). Also: the `gh release create` command in the `release` job uses `GH_TOKEN: ${{ github.token }}`. This requires `permissions: contents: write` at the workflow level, which is correctly specified. No blocker.
- [x] Cross-task conflicts: New file. No conflicts with ci.yml. Both workflows use the same action SHAs — no divergence possible since Task 1 must be merged first.
- [x] Success criteria: `.github/workflows/release.yml` exists. Pushing `v0.0.0-test` tag triggers the workflow. All three jobs (`validate`, `build` x5, `release`) complete successfully. The GitHub Release `v0.0.0-test` contains 5 archives (4 `.tar.gz`, 1 `.zip`) and `checksums.txt`. Test tag and release are cleaned up afterward per the plan's cleanup commands.

**Actions taken:**
- Edited plan to add `Cargo.lock` prerequisite and `ubuntu-24.04-arm` availability note under Task 7.

---

## Task 8: Verify Windows packaging works (cross-platform gotcha)

- [x] Implicit deps: Depends on Task 7 (already captured — cannot verify without release.yml). Also requires a test run to have actually executed, meaning Task 7's test tag push from the verify step is a prerequisite.
- [x] Missing context: The task focuses on the PowerShell `shell: pwsh` forward-slash issue. There is a second potential issue not mentioned: `Compress-Archive` on Windows will fail silently or with a confusing error if either source file does not exist. Both `phyllotaxis.exe` and `phyll.exe` must be present in the build output. Confirmed: both binaries build from the same `src/main.rs` entry point (two `[[bin]]` entries in Cargo.toml), and a local `cargo build --release` produces both. This is not a hidden blocker.
- [x] Hidden blockers: None beyond requiring Task 7's test run.
- [x] Cross-task conflicts: May modify release.yml. Task 7 creates it. Sequential, no conflict.
- [x] Success criteria: The release asset `phyllotaxis-x86_64-pc-windows-msvc.zip` is present in the test release. Downloading and extracting it locally confirms it contains both `phyllotaxis.exe` and `phyll.exe` with non-zero file sizes.

**Actions taken:**
- None required. Both binaries confirmed to build via local release build.

---

## Task 9: Create .claude/hooks/release-guard.py

- [x] Implicit deps: None from earlier tasks. Fully independent. However, the RUNBOOK.md (Task 13) documents the sentinel file format that `release-guard.py` checks — these two files must agree. The dependency is: Task 13 should be written after Task 9 is finalized, not before. A bead dependency was added: Task 13 → Task 9.
- [x] Missing context: Two things added to the plan via direct edit:
  1. Clarified that `.claude/` does not currently exist (confirmed: directory is absent).
  2. Noted that the global `~/.claude/` settings must not be touched — only the project-level `.claude/settings.json` is created.
  The hook logic itself is described as "taken directly from syllago with no changes." An agent would need to trust this claim or verify by looking at the syllago repo. The full file content is provided verbatim, so the agent does not need to find syllago — the plan is self-contained.
- [x] Hidden blockers: The hook uses `python3` via shebang. Python 3 is required to be present on the user's machine and in Claude Code's execution environment. On the GitHub Actions runner this hook does not run (it's a Claude Code hook, not a CI step). Locally, Python 3 is near-universally available on Linux/macOS dev machines.
- [x] Cross-task conflicts: Creates `.claude/settings.json`. No other task touches this file. No conflict.
- [x] Success criteria: `.claude/hooks/release-guard.py` exists and is executable (`ls -l` shows `-rwxr-xr-x`). `.claude/settings.json` exists with the correct hook registration. From within a Claude Code session at the repo root, attempting `git tag v99.99.99` is blocked with the "BLOCKED:" message. Creating `.release-pending.yml` with `status: prepared` allows the same command to pass.

**Actions taken:**
- Added bead dependency: `phyllotaxis-j41` (Task 13) depends on `phyllotaxis-9gz` (Task 9).
- Edited plan to clarify `.claude/` directory state and global vs. project settings scope.

---

## Task 10: Create releases/TEMPLATE.md

- [x] Implicit deps: None from earlier tasks. Independent. However, the `releases/` directory does not exist yet — the agent must create it. The plan says "Files to create: releases/TEMPLATE.md" which implies creating the directory, but this is not stated explicitly.
- [x] Missing context: The `releases/` directory must be created. On most systems `mkdir -p releases` is needed before writing the file. Most file-writing tools handle this implicitly, but an agent working via `Write` tool or `echo >` will fail if the directory does not exist. Minor gap.
- [x] Hidden blockers: None.
- [x] Cross-task conflicts: Task 13 also creates a file in `releases/` (`RUNBOOK.md`). No conflict — different files. Task 7's release job also reads from `releases/` at runtime (looking for `releases/v{tag}.md`). No conflict.
- [x] Success criteria: `releases/TEMPLATE.md` exists with the exact content from the plan. The `releases/` directory itself is committed (either with the TEMPLATE.md file, or with a `.gitkeep` if the directory would otherwise be empty — but since TEMPLATE.md is being created, no `.gitkeep` needed).

**Actions taken:**
- None required. The directory creation is implied and standard.

---

## Task 11: Create VERSIONING.md

- [x] Implicit deps: None. Fully independent.
- [x] Missing context: VERSIONING.md references `.release-pending.yml` and `releases/TEMPLATE.md`. These files are created in Tasks 9 and 10 respectively. If Task 11 is done before Tasks 9/10, the file will reference things that don't yet exist — acceptable since the document is descriptive. No functional issue.
- [x] Hidden blockers: None.
- [x] Cross-task conflicts: None. New file at repo root, no overlap with any other task.
- [x] Success criteria: `VERSIONING.md` exists at repo root with the table and pre-1.0 convention section. No functional verification needed.

**Actions taken:**
- None required.

---

## Task 12: Add .release-pending.yml to .gitignore

- [x] Implicit deps: None stated. Independent in implementation. However, logically this should be done before Task 9 (release-guard.py) is verified — the verification step involves creating `.release-pending.yml` and confirming it doesn't appear in `git status`. If Task 12 is not done first, the verification creates an untracked file that needs to be manually cleaned up. Low risk, but recommended ordering: Task 12 before Task 9 verification.
- [x] Missing context: The plan provides a "Full updated .gitignore" which is a complete file replacement. Confirming this matches the current `.gitignore` exactly: current `.gitignore` has 13 lines. The plan's version adds a comment block and `.release-pending.yml`. Verified — the plan's full content matches the current file contents plus the new addition. No data loss.
- [x] Hidden blockers: None.
- [x] Cross-task conflicts: The git status at conversation start shows `.gitignore` as modified (`M .gitignore` in working tree). This means there are already uncommitted changes to `.gitignore`. The agent must read the current working-tree state of `.gitignore` before applying Task 12's edit — not just the HEAD version — to avoid overwriting the pending change. This is a real conflict risk.
- [x] Success criteria: `.gitignore` contains `.release-pending.yml`. Running `touch .release-pending.yml && git status` shows the file is NOT listed as untracked. The existing content of `.gitignore` (including any changes already staged or modified in working tree) is preserved.

**Actions taken:**
- None required, but the cross-task conflict with the current working-tree state of `.gitignore` is documented prominently above. The agent must read the file before writing.

---

## Task 13: Document the release process (releases/RUNBOOK.md)

- [x] Implicit deps: The runbook references specific artifacts and behaviors from Tasks 7, 9, and 10. Specifically: the sentinel file format must match what `release-guard.py` checks (`status: prepared`), the `releases/TEMPLATE.md` copy command is referenced, and the release workflow job names (`validate`, `build`, `release`) are referenced. The runbook cannot be finalized until these are all settled. Three bead dependencies were added: Task 13 → Task 7, Task 13 → Task 9, Task 13 → Task 10. The plan's dependency section listed Tasks 9-13 as "independent of each other" — this is incorrect for Task 13.
- [x] Missing context: The runbook's "Full Changelog" URL in TEMPLATE.md uses `https://github.com/OpenScribbler/phyllotaxis/compare/...`. An agent creating this file needs to know the actual GitHub org/repo name. It is `OpenScribbler/phyllotaxis` as seen in the plan. This is correct and present in the template content.
- [x] Hidden blockers: None.
- [x] Cross-task conflicts: Creates `releases/RUNBOOK.md`. Task 10 creates `releases/TEMPLATE.md`. Both in the same directory — no conflict, different files. The `releases/` directory is created by Task 10, so Task 13 benefits from Task 10 running first (though most write tools create intermediate directories automatically).
- [x] Success criteria: `releases/RUNBOOK.md` exists. The sentinel file format in Step 4 (`status: prepared`, `version:`, `date:`, `notes:`) exactly matches what `release-guard.py` expects. The workflow job names in Step 6 match the actual `release.yml`. Read through once top-to-bottom to confirm no step references a non-existent file or command.

**Actions taken:**
- Added bead dependencies: `phyllotaxis-j41` depends on `phyllotaxis-ag7` (Task 7), `phyllotaxis-9gz` (Task 9), and `phyllotaxis-vpz` (Task 10).

---

## Summary

- Total tasks: 13
- Dependencies added: 4
  - Task 6 (Dependabot) → Task 1 (ci.yml must exist with pinned SHAs)
  - Task 13 (RUNBOOK.md) → Task 7 (release.yml job names referenced)
  - Task 13 (RUNBOOK.md) → Task 9 (release-guard.py sentinel format referenced)
  - Task 13 (RUNBOOK.md) → Task 10 (TEMPLATE.md cp command referenced)
- New beads created: 0
- Plan updates made: 2
  - Task 7: Added `Cargo.lock` prerequisite and `ubuntu-24.04-arm` availability note
  - Task 9: Clarified `.claude/` directory does not exist; scoped to project-level settings only
- Success criteria added: 13 (one per task)

### Key Findings by Severity

**High — requires agent action before executing:**
1. Task 12 cross-task conflict: `.gitignore` already has uncommitted working-tree changes. Agent must read the current working-tree file state before writing, not derive it from HEAD.

**Medium — documented, low probability of failure:**
2. Task 7: `ubuntu-24.04-arm` runner availability must be verified for the target repo. Fallback strategy documented in plan.
3. Task 13 was listed as independent of Tasks 7/9/10 — it is not. Dependencies now added.
4. Tasks 3 and 4 should be merged together (Task 3 produces schedule-triggered noise until deny.toml exists). The recommended commit grouping already handles this.

**Low — informational:**
5. Task 2 SHA verification requires `gh` auth or browser access.
6. Task 5 first-run advisory DB download requires network access.
7. Task 10 implicitly requires creating the `releases/` directory (does not exist yet).
8. Both `phyllotaxis` and `phyll` binaries confirmed to build correctly from the dual `[[bin]]` Cargo.toml setup.
