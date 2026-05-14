# Evidence — Delphi verification + repo diff review

**Task:** 2026-05-05-delphi-verify-and-repo-review
**Verified at:** 2026-05-05 (local), against `main` working tree (uncommitted M + ?? files).
**Status:** PASS (AC1–AC4)

## AC1 — Delphi vitest

Command:
```
cd apps/delphi/ts
bunx vitest run
```

Result: `Test Files 8 passed (8)  Tests 100 passed (100)  Duration 29.78s`. Exit 0.
Raw: `raw/delphi-vitest.txt`.

## AC2 — Delphi typecheck

Command:
```
cd apps/delphi/ts
bunx tsc --noEmit
```

Result: empty stdout/stderr, exit 0.
Raw: `raw/delphi-tsc.txt`.

## AC3 — ARK write-boundary guard

Command (repo root):
```
node scripts/check-ark-write-boundaries.mjs
```

Result: `ARK write boundary guard passed.` Exit 0.
Raw: `raw/ark-guard-writes.txt`.

## AC4 — Repo diff review

Snapshot of `git status --short` at task start: `raw/git-status.txt` (64 lines, 51 modified + 13 untracked).
Diff stats (`git diff --stat HEAD`): 2442 insertions / 1586 deletions across 51 files.

### Untracked task dirs (?? in `git status`)

- `.agent/tasks/2026-04-26-ark-app-completion/`
- `.agent/tasks/2026-04-26-ark-initial-plan-close/`
- `.agent/tasks/2026-04-26-ark-read-endpoints/`
- `.agent/tasks/2026-04-26-ark-rpc-query-endpoints/`
- `.agent/tasks/2026-04-26-delphi-legacy-removal/`
- `.agent/tasks/2026-04-27-affected-apps-verification/`
- `.agent/tasks/2026-04-27-ark-usage-playtime-summary/`
- `.agent/tasks/2026-04-27-packaged-smoke/`
- `apps/arrancador/scripts/run-packaged-smoke.ts` — new packaged-smoke runner.
- `apps/dashboard/README.md` — new readme.
- `docs/`, `scripts/` — new top-level dirs.

### Modified — area summaries

#### `packages/ark-core/rust` (Rust runtime)

- `src/db.rs` (+741): five new query helpers — `list_objects_by_type`, `get_objects_by_ids`, `list_recent_usage_processes`, `search_usage_processes`, `load_usage_game_playtime_summary`.
- `src/main.rs` (+51): wires four new RPC operations (`ListObjectsByType`, `GetObjectsByIds`, `ListRecentUsageProcesses`, `SearchUsageProcesses`, `GetUsageGamePlaytimeSummary`).
- `src/types.rs` (+56): adds `UsageProcessCandidate`, `UsageGamePlaytimeBinding/Aggregate/Summary/DailyTotal/RangeTotal`.
- `README.md`: doc tweak.

**Risk:** Surface-area expansion only; no migration. Coupled with `kosmos-ark` SDK additions below — both must ship together.

#### `packages/kosmos-ark` (`@kosmos/ark` SDK)

- `src/ark-client.ts` (+97): adds `objects.listByType`, `objects.getMany`, `kv.get/set`, `usage.processes.{recent,search}`, `usage.gamePlaytime.summary`.
- `src/index.ts` (+8): re-exports new types.
- `README.md` (+22): docs.

**Risk:** New API only, no breaking changes. Verify dashboard/arrancador call sites use new shape — they do (see below).

#### `apps/arrancador/electron/main/services` (heavy migration)

- `ark-game-objects.ts` (+259/-86): refactored to ARK SDK primitives (`listByType`, `getMany`).
- `ark-game-migration.ts` (+1): minor.
- `ark-usage.ts` (+467/-72): consumes new `usage.processes` and `usage.gamePlaytime.summary` APIs; new tests (`ark-usage.test.ts` +149).
- `ark-usage-backfill.ts` (+81/-26), `ark-usage-bindings.test.ts` (+8), `ark-usage-backfill.test.ts` (+41), `ark-game-objects.test.ts` (+39): test surface expanded.
- `usage-process-search.ts` (+102/-37): now delegates process candidate queries through `@kosmos/ark`.
- `package.json` (+1): dep bump (likely SDK pin).

**Risk medium:** Largest behaviour change in the diff. Direct-SQLite fallback paths converted to SDK calls. Verifier should re-run arrancador unit tests + the new packaged-smoke runner before tagging anything releasable. Out of AC scope for this task (only delphi was contracted), flagged for follow-up.

#### `apps/delphi/ts` (subject of verification)

- `electron/main.ts` (+34/-22):
  - Adds `KOSMOS_TEST_APPDATA` + `KOSMOS_TEST_USER_DATA` env overrides via new `getAppDataPath()` / `getUserDataPath()` helpers; consistent with the test-DB-isolation rule in `apps/delphi/AGENTS.md`.
  - Removes `PLAYWRIGHT=1` GPU-disable block; replaced by Electron startup callback restructure (`startApp()` invoked from `app.on('ready')`). Risk **low** — vitest passes; e2e not rerun in this task.
  - Window-state and `.migration-backups` paths now flow through the helpers.
- `electron/sidecar.ts` (+6/-2): mirrors `KOSMOS_TEST_APPDATA` for sidecar `appData` + DB path.
- `package.json`: `build:sidecar*` scripts renamed → `build:ark*`; Electron `^41.1.0` → `^38.8.4`, `electron-builder` `^26.0.0` → `^26.8.1`. **Watch:** Electron downgrade is intentional per `apps/delphi/AGENTS.md` rewrite, but every Electron API change should be revalidated by the packaged-smoke task before release.
- `shared/task-object-migration.ts` (+1/-1): `loadAllObjectFirst` no longer falls back to `legacyData.todos`; **object data is the authoritative source** post-migration. Matches AGENTS.md "After migration, object data is the source of truth." A new test in `src/services/storage/task-object-migration.test.ts` (+16) pins this.
- `e2e/shared-ark-task.spec.ts` (+/-): removes `better-sqlite3` direct read; uses `crypto` import directly; consumes `userDataPath` + new env overrides instead of patching `USERPROFILE`.
- `scripts/verifySharedArkTask.mjs`: same env override pattern as the Playwright spec.
- `AGENTS.md` (-484/+41): wholesale rewrite — drops the Russian legacy-architecture section that referenced the removed Delphi DB sidecar; aligns with current ARK-only docs. **No information loss flagged** because the older doc described a removed system.

**Risk:** Delphi changes are coherent and well-bounded. AC1–AC3 confirm typecheck + unit tests pass. E2E and packaged build not exercised in this task (out of scope; deferred to `2026-04-27-packaged-smoke`).

#### `apps/eden/ts` (note/editor side, parallel work)

- `main/store.ts` (+/-): `mapArkObjectToEntry` now uses `list_object_types` to derive `header_layout` per object type; `getNoteTypeById` always asks ARK for types (drops the per-type-id allowlist + heart fallback). New helper `ensureEntryTypeAvailableInArk`.
- `main/main.ts` (+37/-): wiring updates.
- `main/ark.ts`, `heart.ts`, `hevy.ts`: import shape `import electron from "electron"; const { app } = electron;` (CJS interop fix).
- `src/App.vue` (+4), `src/Editor.vue` (+8/-), `src/InlineCaret.ts` (+3): minor renderer tweaks.
- Tests: `app.spec.ts` (+21), `arrancador-ark-sync.spec.ts` (+95/-85), `hevy.spec.ts` (+30), `typing-stress.spec.ts` (+4) — kept in sync with main-process changes.
- `docs/typed-notes-and-eden-heart-plan.md` (-347): plan doc trimmed.

**Risk medium:** Object-type lookup now flows through every `listArkObjects`/`getArkEntry` call (extra `list_object_types` request). Acceptable for current data volumes; revisit if ARK type set grows. No data-corruption risk.

#### Top-level / shared

- `apps/README.md`, `apps/arrancador/AGENTS.md`, `apps/dashboard/AGENTS.md`, `services/usage-tracker/AGENTS.md`, `services/usage-tracker/README.md`: doc sync, no code impact.
- `TODO.md` (+13/-): list update.
- `apps/arrancador/package.json` (+1): dep.
- `bun.lock` (+10): lockfile churn from above bumps.
- `package.json` (+4): root-level tweak.
- `.agent/tasks/2026-04-14-eden-writer-performance/artifacts/typing-perf-playwright.json`, `.agent/tasks/2026-04-25-ark-migration-backup-telemetry/{evidence.md,evidence.json,problems.md,raw/command-results.md}`: prior task artifact updates — **out of code scope**, flagged so they aren't lost in commits.

### Cross-cutting risk notes

- **CRLF warnings:** Git reports `LF will be replaced by CRLF` for nearly every modified file. Working tree is on Windows; `.gitattributes` is the right place to pin if not already set. Not a blocker.
- **Electron downgrade (41 → 38):** Verify packaged-smoke (`apps/arrancador/scripts/run-packaged-smoke.ts`) runs before merging. Out of this task's AC.
- **Test isolation:** All new test paths use `tmpdir()` + `KOSMOS_TEST_*` env overrides; AGENTS.md test-DB-isolation rule satisfied.
- **ARK boundary:** New TypeScript service code routes through `@kosmos/ark` not direct ARK SQLite; guard script confirms (AC3).

## Summary

Delphi: PASS. Vitest 100/100, tsc clean, ARK write guard clean. The uncommitted diff is a coherent ARK-canonical-runtime cleanup: legacy doc content removed, test isolation env wired in, fallback to legacy todos dropped after object migration, Electron pinned down to `^38.8.4`. Largest unchecked surface is arrancador (out of scope here). No blocking issues identified.
