# Evidence - Arrancador hero button alignment

## Scope

- `apps/arrancador/src/pages/GameDetail.tsx`
- `apps/arrancador/src/test/game-detail.test.tsx`
- `.agent/tasks/2026-04-20-arrancador-hero-button-alignment/spec.md`

## What changed

- Moved the primary play button from the separate content block below the hero into the hero footer.
- Aligned the button with the hero title block on the same lower row.
- Kept the missing-install helper text attached to the action area inside the hero.
- Updated the targeted game detail test to assert that the play button now lives inside the hero action block.

## Acceptance criteria

- `AC1` PASS: the primary action now renders inside the hero footer in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:822).
- `AC2` PASS: the hero footer now uses a shared lower row with copy on the left and actions on the right in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:817).
- `AC3` PASS: the targeted test now checks hero action placement in [game-detail.test.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/game-detail.test.tsx:114).
- `AC4` PASS: `bun run typecheck` and the focused game-detail vitest run both passed.

## Verification

- `bun run typecheck` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-detail.test.tsx` PASS

## Notes

- The vitest run still prints a jsdom `Window.alert()` warning to stderr while exiting with `EXIT:0`; this is pre-existing test-environment noise, not a failure.
