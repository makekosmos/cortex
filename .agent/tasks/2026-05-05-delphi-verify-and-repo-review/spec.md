# Delphi verification + repo diff review

**Task ID:** 2026-05-05-delphi-verify-and-repo-review
**Date:** 2026-05-05
**Owner:** repo-task-proof-loop

## Goal

Verify Delphi (Electron + Vue task app, `apps/delphi/ts`) is in working condition against the current code state, and produce a review of the uncommitted repo changes (M + ?? per `git status`).

## Scope

- Delphi: lint-free build, vitest unit tests, TypeScript typecheck.
- Repo-wide ARK write-boundary guard (`bun run ark:guard:writes`) since data-layer files are modified across `apps/arrancador/electron/main/services/*`, `apps/eden/ts/main/*`, `apps/delphi/ts/electron/*`, `packages/ark-core/rust/src/*`, `packages/kepler-ark/src/*`.
- Review of all `M` and `??` paths reported by `git status` at task start: classify by area, summarise intent of the change cluster, and flag risk.

## Out of scope

- Full Electron/Playwright e2e run (requires display + binary builds; ARK Rust sidecar build is heavy). Note as deferred.
- Building `ark-core-rpc` (cargo) for packaged smoke; verifier judges the TypeScript surface only.
- Fixing any defects found — fixer pass would be a separate task.

## Acceptance criteria

- **AC1** Delphi vitest: `cd apps/delphi/ts && bunx vitest run` exits 0. Capture full output to `raw/delphi-vitest.txt`.
- **AC2** Delphi typecheck: `cd apps/delphi/ts && bunx tsc --noEmit` exits 0. Capture to `raw/delphi-tsc.txt`.
- **AC3** ARK write-boundary guard: `node scripts/check-ark-write-boundaries.mjs` exits 0. Capture to `raw/ark-guard-writes.txt`.
- **AC4** Repo diff review: `evidence.md` contains a per-area summary of every modified path, with risk notes. `git status --short` snapshot saved to `raw/git-status.txt`.

## Verification

Verifier reruns AC1–AC3 against current code and confirms AC4 review file exists with full coverage.
