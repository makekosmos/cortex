# Evidence - Arrancador hero visual refresh

## Scope
- `apps/arrancador/src/pages/GameDetail.tsx`
- `apps/arrancador/src/test/game-detail.test.tsx`
- `.agent/tasks/2026-04-20-arrancador-hero-visual-refresh/spec.md`

## What changed
- Removed the dotted background layer from the Arrancador game detail page.
- Extended the game detail hero to a viewport-scaled `80vh` treatment with rounded bottom corners.
- Moved the game title and genre copy into the hero footer at the lower-left corner.
- Reused `kosmos-visuals` game-poster pattern classes for the hero copy so the title/eyebrow language matches the shared game cards.
- Added a targeted test that locks the tall rounded hero and lower-left copy layout.

## Acceptance criteria
- `AC1` PASS: the dotted background layer was removed from [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:769).
- `AC2` PASS: the hero now uses a viewport-scaled height and rounded bottom corners in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:784).
- `AC3` PASS: title and genres now render inside the hero footer instead of below the hero in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:822).
- `AC4` PASS: the hero copy reuses shared game-card visual classes from `kosmos-visuals` in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:31).
- `AC5` PASS: targeted layout coverage was added in [game-detail.test.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/game-detail.test.tsx:114).
- `AC6` PASS: targeted Arrancador verification passed via `bun run typecheck` and `bunx vitest ... game-detail.test.tsx`.

## Verification
### Fresh commands
- `bun run typecheck` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-detail.test.tsx` PASS

### Additional non-gating check
- `bun run build:renderer` BLOCKED by the local environment: Vite/Tailwind native dependency loading (`@tailwindcss/oxide-win32-x64-msvc`) plus `spawn EPERM` in the current machine context. This failure is recorded but was not introduced by the UI change.

## Raw artifacts
- `.agent/tasks/2026-04-20-arrancador-hero-visual-refresh/typecheck.log`
- `.agent/tasks/2026-04-20-arrancador-hero-visual-refresh/game-detail-vitest.log`
- `.agent/tasks/2026-04-20-arrancador-hero-visual-refresh/build-renderer.log`

## Notes
- The `vitest` log contains a jsdom `Window.alert()` warning on stderr while still exiting with `EXIT:0`; the test file passed.
- The `typecheck` log also exits with `EXIT:0`; Bun writes its command banner through stderr in this environment.
