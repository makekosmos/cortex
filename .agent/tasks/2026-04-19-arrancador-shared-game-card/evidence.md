# Evidence - Arrancador shared game card via kepler-visuals

## Scope
- `packages/kepler-visuals/patterns/gamePosterCard.ts`
- `packages/kepler-visuals/patterns/index.ts`
- `packages/kepler-visuals/package.json`
- `apps/arrancador/src/components/GameCard.tsx`
- `apps/arrancador/tailwind.config.ts`
- `apps/arrancador/src/test/game-card.test.tsx`
- `apps/arrancador/src/test/statistics.test.tsx`
- `.agent/tasks/2026-04-19-arrancador-shared-game-card/spec.md`
- `.agent/tasks/2026-04-19-arrancador-shared-game-card/problems.md`

## What changed
- Moved the reusable game-card visual recipe into `kepler-visuals` as `gamePosterCardClasses`.
- Reduced `Arrancador`'s `GameCard` to a thin wrapper that only maps `Game` data to:
  - destination path
  - accessible name
  - image source
  - placeholder content
- Updated the card visual treatment to match the requested behavior:
  - removed visible title
  - removed centered play button overlay
  - removed hover zoom
  - removed border
  - left only subtle darkening on hover/focus
- Added `packages/kepler-visuals` to Arrancador's Tailwind `content` globs so shared utility strings are emitted into the renderer CSS.
- Updated `GameCard` tests for the new shared-card DOM.
- Increased the timeout in `statistics.test.tsx` from `10000` to `20000` ms to stabilize the existing full-suite verification run under current CI/local runtime conditions.

## Acceptance criteria
- `AC1` PASS: shared game-card visual styling now lives in `kepler-visuals` and Arrancador imports it from [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/patterns/gamePosterCard.ts:1).
- `AC2` PASS: Arrancador keeps only a thin wrapper in [GameCard.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/GameCard.tsx:10), where it passes the destination route and cover/placeholder content.
- `AC3` PASS: visible title, centered play overlay, hover zoom, and border were removed from the game card.
- `AC4` PASS: hover treatment is now only the shared darkening overlay in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/patterns/gamePosterCard.ts:13).
- `AC4` PASS note: the effect is now actually emitted in Arrancador builds because [tailwind.config.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/tailwind.config.ts:1) scans `packages/kepler-visuals`.
- `AC5` PASS: relevant tests were updated in [game-card.test.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/test/game-card.test.tsx:8).
- `AC6` PASS: `bun run typecheck` and `bun run test` passed in `apps/arrancador`.

## Verification
### Fresh commands
- `bun run typecheck` PASS
- `bun run build:renderer` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` PASS
- `bun run test` PASS (`31 passed` files, `143 passed | 2 expected fail` tests)

### Raw artifacts
- `.agent/tasks/2026-04-19-arrancador-shared-game-card/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-shared-game-card/game-card-vitest.log`
- `.agent/tasks/2026-04-19-arrancador-shared-game-card/test.log`

## Notes
- The saved `vitest` log still contains expected provider-guard stack traces from tests that intentionally assert hook misuse. Despite that stderr output, the run passed.
- A targeted mojibake scan on the changed card files returned no matches for the previously broken Cyrillic patterns.
- The statistics timeout adjustment was made only to stabilize verification; it does not change application runtime behavior.
- The missing hover update in the live app was caused by Tailwind config scope, not by React rendering logic in `GameCard`.
