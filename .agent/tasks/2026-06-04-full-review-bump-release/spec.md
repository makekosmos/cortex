# 2026-06-04 full review, bump, release

## Goal

Review the current large worktree, catch and fix blocking bugs where feasible, record repeated Windows Sandbox / release workflow lessons in repo skills, then patch-bump, commit, push, build, and publish touched release targets.

## Scope

- Review tracked and untracked changes currently present in the repository.
- Fix concrete bugs found during review or verification.
- Preserve user WIP; do not revert unrelated changes.
- Update repo skills/documentation only for durable agent workflow rules.
- Patch-bump release targets touched by the changes unless verification blocks release.
- Build and publish releases through the existing Bun/GH CLI workflows when verification passes.

## Out of scope

- Broad architecture rewrites not required by review findings.
- Minor/major version bumps unless explicitly requested separately.
- Force-push, destructive resets, or deleting published GitHub releases.

## Acceptance Criteria

**AC1.** Review coverage is explicit: the final evidence lists the changed areas inspected and any findings fixed or intentionally left unresolved.

**AC2.** Windows Sandbox friction is recorded in a reusable skill so future agents know when to request escalation instead of retrying wastefully.

**AC3.** Bump semantics are recorded in the bump skill: default bump is patch `+0.0.1`; build and publish are part of bump unless the user explicitly asks otherwise.

**AC4.** Relevant guards/tests/builds are run after fixes, including docs sync/check for docs changes, ARK write guard for data-layer changes, shell typecheck, extension build, and targeted unit/e2e tests where applicable.

**AC5.** If all release-blocking checks pass, every touched user-facing release target is patch-bumped, committed, pushed, built, and published; release artifacts are verified through `gh release view`.

**AC6.** If any release-blocking check cannot pass or required publication credentials/permissions are unavailable, `problems.md` documents the blocker and no false PASS is reported.
