# Evidence

Verified against the current dirty worktree on 2026-07-14. Only the Everything/titlebar/create-note changes described below belong to this acceptance pass; the concurrent book metadata/import work remains separate.

## AC1 — PASS

- `products/eden/src/App.vue` shows the existing «Всё» / «Дневник» switch only for those two main pages. Object pages reuse the existing title presentation in the center slot, including the existing round person image/icon path.
- `products/eden/tests/components/EdenStoreNavigation.spec.ts` still proves cold-start home state and `openEverything()` cleanup.
- `tests/e2e/visual.spec.ts` proves the book page has no main navigation, displays `Море внутри`, and centers its title within one CSS pixel of the viewport center.
- The updated `eden-book-object-layout-win32.png` baseline was inspected at original resolution: the object title is centered and the main-page switch is absent.

## AC2 — PASS

- `EverythingView.spec.ts` proves mixed filtering (`note_obj` / `book_obj` only), `updated_at DESC`, cover/fallback/note variants, search behavior, and `open-entry` payload.
- The view still consumes the already loaded entries and note types; this change adds no data API or hydration path.

## AC3 — PASS

- `products/eden/tests/systemTypes.test.ts` proves `book_obj` registration and its typed cover/author schema.
- `rtk bun run --cwd products/eden test:unit` — PASS: 95 tests.
- `rtk bun run ark:guard:writes` — PASS.

## AC4 — PASS

- `EverythingView.spec.ts` confirms the add card is square and first, the newest card starts to its right, a later card continues below it, narrow width collapses to one column, and existing equal-gap, ellipsis, hover-transition and non-overlap assertions pass.
- The preview test confirms no body read happens before intersection, the observer is rooted in the Everything scroll container with a `50% 0px` overscan, repeated intersection does not duplicate the read, and cached previews refresh only when `updated_at` changes.
- The context-menu test confirms right-click opens the shared Visuals menu, the destructive «Удалить» item contains a trash SVG, and the selected entry id is emitted. The store test confirms successful ARK soft-delete removes only that entry from Everything while a failed write preserves it.
- `eden-everything-context-menu-win32.png` was inspected at original resolution: the flat menu appears at the clicked card with the trash icon and no box-shadow. The Electron flow then deletes the note, observes the menu close, and sees the card count drop from four to three.
- `eden-top-navigation-mixed-win32.png` was inspected at original resolution: the first column continues below `+` instead of remaining artificially empty.

## AC4a — PASS

- The deferred-save store test proves `createNewEntry()` selects the new object synchronously while `entries` remains unchanged, then adds it to Everything only after successful persistence.
- `App.vue` keeps the editor unmounted behind the existing loading presentation until that primary save completes, preserving the established empty-save/autosave ordering.
- The Electron flow clicks `+`, immediately observes an object page with no main navigation and a visible editor after save, then goes Back and observes one additional Everything card.
- The first clean Electron rerun exposed a live-refresh race: an ARK upsert begun before the primary save could prepend the same ID after `createNewEntry()` had already inserted it. `edenLiveRefreshSubscription.ts` now reuses the store's id-aware upsert, and `EdenLiveRefreshSubscription.spec.ts` deterministically covers that ordering.

## AC5 — PASS

- Source documentation and the frozen task spec describe the titlebar scope, first-column add card, and immediate-create ordering.
- `rtk bun run --cwd platform/desktop build:js` — PASS, including Eden's production bundle.

## Verification

- `rtk bun run --cwd products/eden test:unit` — 95 passed.
- `rtk bun run --cwd products/eden test:vue` — 91 passed across 16 files.
- `rtk bun run ark:guard:writes` — PASS.
- `rtk bun run --cwd platform/desktop build:js` — PASS.
- `rtk bun run visual:eden --update-snapshots` — PASS; changed baselines were visually inspected at original resolution.
- `rtk bun run visual:eden` — PASS without snapshot updates after the live-refresh fix: 1/1, including create → Back with exactly one new card.
- `rtk bunx oxfmt --check <12 scoped files>` and `rtk git diff --check` — PASS.
- `rtk bun run docs:check` — PASS.

## Independent verifier

The first read-only review correctly rejected the implementation after reproducing the in-memory duplicate and identifying the live-refresh race. The second read-only review returned PASS: focused navigation/Everything/race tests 14/14 and `visual:eden` 1/1; it independently confirmed exactly one new card after create → Back, scoped main navigation, centered object title, and the add-card column break. Person image/icon and Forward remain code-reviewed reuse paths rather than separate runtime assertions.
