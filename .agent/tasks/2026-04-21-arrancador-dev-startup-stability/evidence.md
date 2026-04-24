# Evidence: Arrancador dev startup stability and blank-screen regression

## Current Status

PASS

## Delivered scope

- Dev Windows shell no longer uses the old focus-sensitive startup path:
  - `apps/arrancador/electron/main/windows.ts`
  - dev windows now start hidden, avoid the custom Windows titlebar path in dev, and no longer force focus during automatic startup reveal callbacks
- Dev startup now prewarms the first renderer modules before Electron launches:
  - `apps/arrancador/scripts/dev.ts`
- Router pages are lazy-loaded instead of being eagerly imported:
  - `apps/arrancador/src-vue/router.ts`
- The renderer now shows an inline startup placeholder instead of a plain blank shell:
  - `apps/arrancador/index.html`
  - `apps/arrancador/src-vue/main.ts`
- Duplicate initial library refresh was removed:
  - `apps/arrancador/src-vue/pages/LayoutPage.vue`
  - `apps/arrancador/src-vue/pages/LibraryPage.vue`
- Because dev Windows now uses the native frame for stability, the in-app custom titlebar is hidden for that dev-only path:
  - `apps/arrancador/src-vue/pages/LayoutPage.vue`

## Acceptance Criteria status

- AC1: PASS
  - `windows.ts` no longer combines dev early show + custom Windows titlebar handling + forced focus during startup callbacks.
- AC2: PASS
  - `router.ts` now lazy-loads route pages.
- AC3: PASS
  - `index.html` renders a visible startup placeholder and `main.ts` removes it after Vue mounts.
- AC4: PASS
  - Initial game refresh no longer runs from both layout and library on first mount.
- AC5: PASS
  - Fresh `typecheck`, `test`, `build:renderer`, and `build:main` passed.

## Verification run

- `bun run typecheck` -> PASS
- `bun run test` -> PASS
- `bun run build:renderer` -> PASS
- `bun run build:main` -> PASS

## Notes

- I could not perform a real GUI `Alt+Tab` reproduction end-to-end inside this environment, so the Alt+Tab portion is fixed from code-path analysis plus local Electron startup investigation rather than from interactive desktop validation.
