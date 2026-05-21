# Problems - Arrancador horizontal game card

## Verification blocker

- `npx vite build` still fails in `apps/arrancador` before renderer bundling completes.
- The captured log in `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/build-renderer.log` shows two environment-level issues while loading the Vite config path:
  - `@tailwindcss/oxide-win32-x64-msvc` native binding cannot be loaded and reports `stream did not contain valid UTF-8`.
  - Vite's config dependency externalization hits `spawn EPERM` on Windows path resolution.

## Smallest safe fix attempted

- Kept the product diff limited to the shared card, Arrancador card adapter, grid density, and the existing delegation test.
- Avoided broad Vite/Tailwind config surgery because the failure occurs in the existing native build pipeline and is not isolated to the horizontal card change.

## Current status

- `bunx tsc --noEmit` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` PASS
- `npx vite build` FAIL due the existing environment/config-loader blocker above
