# Evidence

Verified against the final current worktree at `2026-07-13T14:10:44.3375842Z`. The pre-existing modification in `platform/desktop/package.json` was not edited as part of this task.

## AC1 — PASS

- `products/eden/tests/components/EdenStoreNavigation.spec.ts` proves stale `eden:nav:lastScreen` / `eden:nav:lastEntryId` keys are ignored and `openEverything()` clears the screen, collection, entry and pending loading state.
- `products/eden/tests/components/EdenSidebar.spec.ts` proves «Всё» is present, activatable and active only for the home state.
- `rtk bun run --cwd products/eden test:vue` — PASS: 15 files, 78 tests.
- `rtk bun run visual:eden` — PASS: after opening a book, the enabled titlebar Back action returns to `everything-view`.

## AC2 — PASS

- `EverythingView.spec.ts` proves mixed filtering (`note_obj` / `book_obj` only), `updated_at DESC`, cover/fallback/note variants, Russian empty state and `open-entry` payload.
- The view receives only `entries` / `noteTypes`; production build and diff contain no new API, RPC or body hydration path.
- `rtk bun run --cwd products/eden test:vue` — PASS: 78/78.

## AC3 — PASS

- `products/eden/tests/systemTypes.test.ts` proves `book_obj` registration, optional `cover_image:image`, optional `author:text`, `imageFieldId`, collection «Книги», icon `book` and `color: null`.
- `rtk bun run --cwd products/eden test:unit` — PASS: 95/95.
- `rtk bun run ark:guard:writes` — PASS: ARK write boundary guard passed.
- No SQLite migration, RPC, sync protocol or manifest file is present in the task diff.

## AC4 — PASS

- Browser layout assertions prove equal computed `column-gap` / card bottom margin, `break-inside: avoid`, one narrow column and non-overlapping card rectangles.
- `rtk bun run visual:eden -- --update-snapshots` — PASS: created `tests/e2e/visual.spec.ts-snapshots/eden-everything-mixed-win32.png` from cold-reopened Eden with two deterministic covers, a fallback book and a note.
- The baseline was opened at original resolution and visually inspected: natural cover ratios, fallback, three-column rhythm, sidebar home state and hidden reader toggle are correct.
- `rtk bun run visual:eden` — PASS without update: 1/1 in 4.4 s.

## AC5 — PASS

- `App.vue` only composes store actions and typed component events; cards contain no ARK bridge calls.
- `rtk bun run --cwd platform/desktop build:js` — PASS, including Eden extension production bundle.
- `rtk bun run docs:sync` — PASS.
- `rtk bun run docs:check` — PASS: generated docs fresh, no stale references.

## AC6 — PASS

- `rtk bun run --cwd products/eden test:unit` — 95 passed, 0 failed.
- `rtk bun run --cwd products/eden test:vue` — 78 passed, 0 failed.
- `rtk bun run ark:guard:writes` — PASS.
- `rtk bun run --cwd platform/desktop build:js` — PASS.
- `rtk bun run docs:sync` and `rtk bun run docs:check` — PASS.
- Visual update and a clean no-update rerun — PASS.
- `rtk bunx oxfmt --check <17 changed implementation/test/doc files>` — PASS.
- `rtk git diff --check` — PASS.

The resolved screenshot-capture failure and the non-scope stale editor test are recorded in `problems.md`.

## Independent verifier

Independent read-only verification after this evidence was written returned `AC1`–`AC6: PASS` with no actionable findings. Its fresh run repeated unit 95/95, Vue 78/78, ARK guard, desktop build, docs check, visual no-update 1/1 and `git diff --check`; it also inspected the baseline at original resolution and confirmed the pre-existing `platform/desktop/package.json` diff remained outside task scope.
