# Evidence

Task ID: `2026-04-23-arrancador-route-atomization`

## Verification Result

Overall: `PASS`

## Acceptance Criteria

AC1. Library filtering and sorting pure logic is extracted from `LibraryPage.vue`.

Result: `PASS`

Evidence:
- Added `apps/arrancador/src-vue/lib/libraryFilters.ts`.
- Moved filter preset sanitization, numeric parsing, metadata list splitting, playtime formatting, metadata option extraction, active-filter counting, and game filtering/sorting into the module.
- `LibraryPage.vue` now delegates genre/platform options, active-filter count, and filtered game derivation to this module.

AC2. Game detail display helpers are extracted from `GameDetailPage.vue`.

Result: `PASS`

Evidence:
- Added `apps/arrancador/src-vue/lib/gameDetailDisplay.ts`.
- Moved playtime/played-hours/bytes formatting, description normalization, save-path template resolution, and play-status labels/tones into the module.
- `GameDetailPage.vue` imports and uses the centralized helpers/constants.

AC3. Extracted logic has focused unit tests.

Result: `PASS`

Evidence:
- Added `apps/arrancador/src-vue/test/library-filters.test.ts`.
- Added `apps/arrancador/src-vue/test/game-detail-display.test.ts`.
- Test suite now runs `36` tests across `13` files.

AC4. Existing behavior remains verified.

Result: `PASS`

Evidence:
- `bun run typecheck`: pass.
- `bun run test`: pass, `13 passed (13)`, `36 passed (36)`.
- `bun run biome:check`: pass.
- `bun run test:e2e`: attempted; app build completed, Playwright execution blocked by local `spawn EPERM` when launching worker/browser process. Blocker is environment-level and recorded in `raw/test-e2e.txt`.

AC5. Proof artifacts are recorded.

Result: `PASS`

Evidence:
- Raw artifacts:
  - `raw/typecheck.txt`
  - `raw/test.txt`
  - `raw/biome-check.txt`
  - `raw/test-e2e.txt`

## Notes

- This pass reduces route-page responsibility without altering the rendered UI structure.
- The remaining largest route component is still `GameDetailPage.vue`, but its pure display helpers are now isolated and tested. A later pass can split its backup, metadata, rating, and path-edit sections into child components/composables.
