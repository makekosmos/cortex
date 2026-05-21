# Evidence

Task ID: `2026-04-23-arrancador-quality-hardening`

## Verification Result

Overall: `PASS`

## Acceptance Criteria

AC1. Mojibake is removed from active Arrancador source and docs touched by this task.

Result: `PASS`

Evidence:

- Added `apps/arrancador/src-vue/test/source-encoding.test.ts`.
- The test scans active Arrancador source/docs as UTF-8 and rejects common mojibake marker sequences.
- Raw encoding gate is included in `bun run test`.
- Manual scan result: `[]`.

AC2. The default `bun run test` command runs both renderer and Electron main unit tests.

Result: `PASS`

Evidence:

- `apps/arrancador/vitest.config.mjs` now defines separate Vitest projects:
  - `renderer`: jsdom, Vue setup, `src-vue/test/**/*`
  - `electron-main`: node, `electron/main/**/*.test.ts`
- Electron main tests now import `vitest` instead of `bun:test`.
- `bun run test`: `11 passed (11)`, `25 passed (25)`.

AC3. Shared pure game-import/path logic is extracted from large route pages.

Result: `PASS`

Evidence:

- Added `apps/arrancador/src-vue/lib/gameImport.ts`.
- `LibraryPage.vue` and `ScanPage.vue` import shared helpers instead of defining duplicate local functions.
- `rg` finds helper definitions only in `src-vue/lib/gameImport.ts`.

AC4. Test coverage is added for the extracted shared logic.

Result: `PASS`

Evidence:

- Added `apps/arrancador/src-vue/test/game-import.test.ts`.
- Tests cover supported drop paths, filename extraction, display-name cleanup, normalization, fuzzy merge matching, and merge-candidate lookup.

AC5. Verification is fresh and recorded.

Result: `PASS`

Evidence:

- Raw artifacts:
  - `raw/typecheck.txt`
  - `raw/test.txt`
  - `raw/biome-check.txt`
  - `raw/test-e2e.txt`
- `bun run typecheck`: pass.
- `bun run test`: pass.
- `bun run biome:check`: pass.
- `bun run test:e2e`: attempted; production build completed, Playwright execution blocked by local `spawn EPERM` for worker/browser process. This is recorded as an environment blocker allowed by AC5.

## Notes

- Earlier terminal output displayed mojibake through PowerShell rendering, but Node UTF-8 reads confirmed source files contain correct Russian strings. The new encoding test guards the source files rather than terminal rendering.
- E2E remains blocked by local process execution permissions, not by app build failure.
