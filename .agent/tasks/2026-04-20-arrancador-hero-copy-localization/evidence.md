# Evidence - Arrancador hero copy localization and metadata layout

## Scope
- `apps/arrancador/src/pages/GameDetail.tsx`
- `apps/arrancador/src/test/game-detail.test.tsx`
- `.agent/tasks/2026-04-20-arrancador-hero-copy-localization/spec.md`

## What changed
- Added a local English-to-Russian genre mapping for the hero copy.
- Reordered the hero text stack to: title, genres, short description, then a metadata row.
- Added short-description summarization and a compact played-hours formatter for the hero.
- Updated the targeted game detail test to assert Russian genre labels, short description, and `year · hours` metadata.

## Acceptance criteria
- `AC1` PASS: hero genres are translated to Russian via local mapping in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx:45).
- `AC2` PASS: the hero copy now renders title, genres, short description, and metadata in that order in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx:851).
- `AC3` PASS: the focused game detail test was updated in [game-detail.test.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/test/game-detail.test.tsx:101).
- `AC4` PASS: `bun run typecheck` and the focused `vitest` run both passed.

## Verification
- `bun run typecheck` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-detail.test.tsx` PASS

## Notes
- The `vitest` run still prints a jsdom `Window.alert()` warning on stderr while exiting with `EXIT:0`; this is existing test-environment noise, not a failing assertion.
