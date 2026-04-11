# Evidence: Delphi TS P2P Sync Cleanup

## Verification Summary

- AC1: PASS
  In Electron P2P mode, renderer mutations no longer go through `broadcastToPeers` or any `peer:*` bridge. Local changes in [todos.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/store/todos.ts:1), [tasks.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/store/tasks.ts:1), and [ProjectPage.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/pages/ProjectPage.vue:1) now use only `lan-sync:broadcastChange`.

- AC2: PASS
  Desktop settings no longer expose Ark Server pairing / connect / disconnect UI. [SettingsPage.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/pages/SettingsPage.vue:1) keeps Spaces and Google Calendar settings, including a visible OAuth config path for packaged Electron builds.

- AC3: PASS
  Electron runtime boundaries are `lan-sync:*` only. [App.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/App.vue:1) starts sync via `lan-sync:start`, leaves via `lan-sync:leaveSpace`, and consumes only `lan-sync:change` / peer events. Removed `peer:*` renderer stubs are absent from [main.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/main.ts:291).

- AC4: PASS
  No removed legacy Electron sync paths remain in Delphi TS. Browser-only Ark bootstrap remains present in [App.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/App.vue:182), [pairing.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/services/sync/pairing.ts:1), [AuthOverlay.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/AuthOverlay.vue:1), and [ark-types.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/services/sync/ark-types.ts:34). TypeScript compilation passed, and targeted tests passed.

## Regression Fixes

- Google Calendar refresh now preserves the latest queued range request while a sync is already running: [google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:103)
- Settings now expose OAuth Client ID / Client Secret persistence through `google-calendar:setConfig`: [SettingsPage.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/pages/SettingsPage.vue:1)
- Non-Electron builds again have an auth/bootstrap path using Ark HTTP fetch helpers: [App.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/App.vue:182)

## Commands

- `bun x tsc --noEmit -p tsconfig.json`
  Result: PASS
  Raw: [typecheck.txt](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/raw/typecheck.txt:1)

- `bun run vitest run src/services/space/__tests__/space-manager.test.ts src/services/sync/__tests__/lan-protocol.test.ts`
  Result: PASS, `2` files / `66` tests passed
  Raw: [tests.txt](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/raw/tests.txt:1)

- `rg -n "broadcastToPeers|peer:*|Ark Server" apps/delphi/ts apps/delphi/AGENTS.md -g '!**/dist/**'`
  Result: PASS, no matches
  Raw: [electron-legacy-search.txt](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/raw/electron-legacy-search.txt:1)

- `rg -n "fetchTasksFromArk|fetchProjectsFromArk|parseConnectionString|AuthOverlay|delphi.ark_*" apps/delphi/ts/src -g '!**/dist/**'`
  Result: PASS, browser bootstrap helpers present where intended
  Raw: [web-bootstrap-search.txt](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/raw/web-bootstrap-search.txt:1)

- `git diff --check -- apps/delphi/ts apps/delphi/AGENTS.md`
  Result: PASS
  Raw: [diff-check.txt](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/raw/diff-check.txt:1)

## Notes

- Review surfaced three regressions after the initial cleanup; they are recorded in [problems.md](/Users/kirill/Documents/projects/kepler/.agent/tasks/2026-04-11-delphi-p2p-sync-cleanup/problems.md:1) and were fixed before this verification pass.
- Unrelated user changes remain in the worktree and were not reverted.
