# Evidence - Arrancador hero description modal

## Scope
- `apps/arrancador/src/pages/GameDetail.tsx`
- `apps/arrancador/src/test/game-detail.test.tsx`
- `.agent/tasks/2026-04-20-arrancador-description-modal/spec.md`

## What changed
- Replaced the character-trimmed hero description with normalized full text data.
- Clamped the hero description to two visible lines.
- Added a hero-side button that opens a darkened modal with the full description text.
- Updated the focused game detail test to verify the clamp class, description button, and modal opening.

## Acceptance criteria
- `AC1` PASS: the hero description now uses two-line clamp styling in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx:875).
- `AC2` PASS: the description button and darkened modal were added in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx:881) and [GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx:1532).
- `AC3` PASS: the targeted modal behavior is covered in [game-detail.test.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/test/game-detail.test.tsx:101).
- `AC4` PASS: `bun run typecheck` and the focused `vitest` run both passed.

## Verification
- `bun run typecheck` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-detail.test.tsx` PASS

## Notes
- The `vitest` run still prints a jsdom `Window.alert()` warning to stderr while exiting with `EXIT:0`; this is existing test-environment noise, not a failing assertion.
