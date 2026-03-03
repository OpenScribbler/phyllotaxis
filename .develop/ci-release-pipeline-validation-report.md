# Validation Report: ci-release-pipeline

**Design doc:** `docs/plans/2026-03-02-ci-release-pipeline-design.md`
**Implementation plan:** `docs/plans/2026-03-02-ci-release-pipeline-implementation.md`
**Validated:** 2026-03-02

---

## Validation Report

### Covered (26 requirements → 13 tasks)

**Architecture components (all 7):**
- CI workflow (`.github/workflows/ci.yml`) → Tasks 1, 3
- Release workflow (`.github/workflows/release.yml`) → Task 7
- Dependabot config (`.github/dependabot.yml`) → Task 6
- cargo-deny config (`deny.toml`) → Tasks 4, 5
- Release notes template (`releases/TEMPLATE.md`) → Task 10
- Versioning policy (`VERSIONING.md`) → Task 11
- Release guard hook (`.claude/hooks/release-guard.py`) → Task 9

**Build matrix (all 5 targets):**
- `x86_64-unknown-linux-gnu` / `ubuntu-latest` → Task 7 matrix
- `aarch64-unknown-linux-gnu` / `ubuntu-24.04-arm` → Task 7 matrix
- `x86_64-apple-darwin` / `macos-13` → Task 7 matrix
- `aarch64-apple-darwin` / `macos-latest` → Task 7 matrix
- `x86_64-pc-windows-msvc` / `windows-latest` → Task 7 matrix

**Pinned SHAs (all 6 match exactly):**
- `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5` → Tasks 1, 7
- `dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9` → Tasks 1, 7
- `Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5` → Tasks 1, 7
- `EmbarkStudios/cargo-deny-action@3fd3802e88374d3fe9159b834c7714ec57d6c979` → Task 1
- `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02` → Task 7
- `actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093` → Task 7

**deny.toml (exact match to design):**
- All 4 sections present: `[advisories]`, `[licenses]`, `[bans]`, `[sources]` → Task 4
- License allowlist identical: MIT, Apache-2.0, Unicode-3.0, Unicode-DFS-2016, BSL-1.0, ISC → Task 4
- `confidence-threshold = 0.8` → Task 4
- `multiple-versions = "warn"`, `wildcards = "allow"` → Task 4
- `allow-registry`, `allow-git = []` → Task 4

**Error handling scenarios (all 5):**
- Tag/Cargo.toml version mismatch → validate job in Task 7 (clear error + exit 1)
- Build failure on one target → matrix isolation in Task 7; recovery steps in Task 13
- Missing release notes file → `--generate-notes` fallback in Task 7
- cargo-deny finds license issue → deny job in Task 1 blocks CI
- cargo-audit finds vulnerability → audit job in Task 1 blocks CI

**Success criteria (all 7):**
- CI runs on every push/PR with pinned SHAs and permissions blocks → Task 1
- cargo-deny checks licenses, advisories, and duplicates → Tasks 1, 4
- Tag push builds binaries for all 5 targets → Task 7
- GitHub Release created with archives and checksums.txt → Task 7 release job
- Tag version must match Cargo.toml version → Task 7 validate job
- Release guard hook prevents accidental tag creation → Task 9
- Dependabot auto-PRs for action and cargo updates → Task 6

**Orphan task check (all 13 tasks trace to design requirements):**
- Task 2 (verify SHAs) → design: "Pinned Action SHAs (to be verified at implementation time)"
- Task 3 (deny schedule) → design: audit runs on weekly schedule; deny is logically identical
- Task 8 (Windows packaging) → design: cross-platform archive requirement, platform conventions note
- Task 12 (.gitignore) → design: ".release-pending.yml is deleted and .gitignored"
- Task 13 (RUNBOOK.md) → design: Release Flow section (8-step process)

**Files not in architecture table but clearly implied by design:**
- `.claude/settings.json` → required for release-guard.py registration (Task 9)
- `releases/RUNBOOK.md` → operationalizes the Release Flow section (Task 13)

---

### Gaps Found (1 issue)

1. **Vague/undecided task description (Task 3):** The original Task 3 said "if you decide X, this task is a no-op" and left the schedule decision to implementation time. This violated the plan's own standard — tasks should not contain TBD decisions. The design's audit job runs on schedule to catch new advisories without code changes; deny should do the same for license violations. Decision: remove the `if:` guard from the deny job so it runs on schedule.

---

### Action Required

**Gap fixed.** Task 3 was rewritten to make the decision explicit: remove the `if: github.event_name != 'schedule'` guard from the deny job so it runs on the weekly schedule alongside audit. The "if you decide otherwise, this is a no-op" hedge was removed. Task description now states the reasoning directly.

No other gaps found. All 7 architecture components, all 5 build targets, all 6 pinned SHAs, the complete deny.toml, all 5 error handling scenarios, and all 7 success criteria have corresponding tasks. No orphan tasks exist.

Proceed to Beads creation.
