# Evidence: Eden diary cache lifecycle

Verified against the current Windows worktree on 2026-07-13.

> Follow-up: a later explicit user request expanded the original one-entry diary cache to `<KeepAlive :max="2">` for both `EverythingView` and `BubbleDiaryView`. The original frozen AC and verification below remain as historical evidence; the current two-page cache is covered by the follow-up regression described at the end.

## Acceptance criteria

- **AC1 — PASS.** `App.vue` keeps the lazily created `BubbleDiaryView` inside `<KeepAlive :max="1">`. The browser regression test records zero reads before the first open and no additional `list_objects_by_type` read after closing and reopening.
- **AC2 — PASS.** The regression test emits `object_upserted` while the cached diary is inactive, observes the subscription-triggered refresh, then reopens the same instance and sees the new bubble without another navigation-triggered read.
- **AC3 — PASS.** The cache has `max=1`; first mount remains conditional on `activeScreen === "diary"`. `BubbleDiaryView` cleanup code is unchanged and still runs when the cached instance is finally unmounted with Eden.
- **AC4 — PASS.** No cache store, dependency, API, schema or write-path was added. `ark:guard:writes` passes.
- **AC5 — PASS.** Unit, browser, targeted Eden build, docs, format and Electron visual checks pass.

## Commands

| Command                                                                     | Result                                               |
| --------------------------------------------------------------------------- | ---------------------------------------------------- |
| `rtk bun run --cwd products/eden test:unit`                                 | PASS — 95 tests                                      |
| `rtk bun run --cwd products/eden test:vue`                                  | PASS — 75 tests                                      |
| `rtk bun run ark:guard:writes`                                              | PASS                                                 |
| `rtk node scripts/build-extensions.mjs --only eden` from `platform/desktop` | PASS                                                 |
| `rtk bun run docs:sync`                                                     | PASS                                                 |
| `rtk bun run docs:check`                                                    | PASS                                                 |
| `rtk bunx oxfmt --check ...` (changed diary-cache files)                    | PASS                                                 |
| `rtk bun run visual:eden`                                                   | PASS — 1 Electron test against the existing baseline |
| `rtk git diff --check`                                                      | PASS                                                 |

## Scope guard

`platform/desktop/package.json` remains an unrelated pre-existing `dev:akashi` change and was not edited for this task.

## Independent verification

The orchestrator reviewer independently returned **AC1–AC5 PASS** with no actionable or blocking findings. Its fresh targeted checks passed: 9/9 `BubbleDiaryView` browser tests, ARK write guard, targeted Eden build, docs freshness and `git diff --check`.

## Two-page cache follow-up

The updated browser regression preserves the same `EverythingView` DOM instance, applies reactive entry updates while it is inactive, preserves the diary instance and applies diary subscription updates while hidden. Targeted browser tests pass 9/9, the full browser suite passes 75/75, and the independent reviewer returned PASS with no blocking findings.
