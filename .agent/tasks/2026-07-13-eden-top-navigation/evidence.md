# Evidence: Eden top navigation

Verified against the current Windows worktree on 2026-07-13.

## Acceptance criteria

- **AC1 — PASS.** `rg` finds no remaining `EdenSidebar`, `RecentSidebarItem`, `widgetSidebar*`, sidebar toggle or sidebar context-menu references in Eden UI/tests. The sidebar SFCs and their dedicated browser spec are deleted.
- **AC2 — PASS.** `App.vue` renders one accessible `nav` with exactly two native buttons, test ids `top-nav-everything` / `top-nav-diary`, `aria-current="page"` on the active screen, and a focus-mode guard. The Electron baseline visually confirms the centered titlebar control.
- **AC3 — PASS.** The buttons call the existing `eden.openEverything()` / `eden.openDiary()` actions. The headless Electron scenario switches from Everything into a book editor, then to Diary and back to Everything.
- **AC4 — PASS.** Eden-local sidebar state and persistence initialization were removed from `layout.ts` / `edenStoreDataActions.ts`; Ctrl/Cmd+B was removed from `useKeyboard.ts`. Existing unit/browser suites and desktop JS build pass.
- **AC5 — PASS.** Navigation CSS uses existing Eden tokens only. The Electron test asserts app/chrome/body/main each occupy the full viewport width; manual inspection of `tests/e2e/visual.spec.ts-snapshots/eden-top-navigation-mixed-win32.png` found no overlap or clipping. Source UI docs describe the new navigation.
- **AC6 — PASS.** All required commands below passed, including a no-update visual regression run after baseline creation.

## Commands

| Command                                                     | Result                                       |
| ----------------------------------------------------------- | -------------------------------------------- |
| `rtk bun run --cwd products/eden test:unit`                 | PASS — 95 tests                              |
| `rtk bun run --cwd products/eden test:vue`                  | PASS — 74 tests                              |
| `rtk bun run ark:guard:writes`                              | PASS                                         |
| `rtk bun run --cwd platform/desktop build:js`               | PASS                                         |
| `rtk bun run docs:sync`                                     | PASS                                         |
| `rtk bun run docs:check`                                    | PASS                                         |
| `rtk bunx oxfmt --check ...` (changed top-navigation files) | PASS                                         |
| `rtk bun run visual:eden --update-snapshots`                | PASS — 1 Electron test, new baseline written |
| `rtk bun run visual:eden`                                   | PASS — 1 Electron test against baseline      |
| `rtk git diff --check`                                      | PASS                                         |

## Scope guard

`platform/desktop/package.json` remains an unrelated pre-existing one-line `dev:akashi` change and was not edited as part of this task.

## Motion follow-up

The active pill is now one shared CSS indicator, and Everything/Diary use an opacity crossfade without delaying the synchronous store action. The Electron test verifies `aria-current` immediately after each click, observes a real opacity `transitionrun`, and passes against the existing baseline.
