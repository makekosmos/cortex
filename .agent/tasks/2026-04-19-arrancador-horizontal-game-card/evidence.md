# Evidence - Arrancador horizontal game card

## Scope
- `packages/kosmos-visuals/react/GamePosterCard.tsx`
- `packages/kosmos-visuals/patterns/gamePosterCard.ts`
- `apps/arrancador/src/components/GameCard.tsx`
- `apps/arrancador/src/pages/Library.tsx`
- `apps/arrancador/src/test/game-card.test.tsx`
- `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/spec.md`

## What changed
- Converted the shared `kosmos-visuals` game card from a portrait poster into a horizontal media card with `aspect-[16/9]` in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/patterns/gamePosterCard.ts:1).
- Added a permanent bottom scrim plus a text layer for eyebrow/title in the shared React component at [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:10).
- Switched Arrancador card image priority to `background_image || cover_image` and derived the first genre string as the eyebrow in [GameCard.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/GameCard.tsx:11).
- Reduced the library grid density so horizontal cards have enough visual width in [Library.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/Library.tsx:1346).
- Updated the Arrancador card delegation test to assert the new wide-image preference and eyebrow contract in [game-card.test.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/game-card.test.tsx:8).

## Acceptance criteria
- `AC1` PASS: the shared card now uses a horizontal aspect ratio via `aspect-[16/9]` in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/patterns/gamePosterCard.ts:3).
- `AC2` PASS: Arrancador now prefers `background_image` and falls back to `cover_image` in [GameCard.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/GameCard.tsx:12).
- `AC3` PASS: the card always renders a bottom darkening scrim through `gamePosterCardClasses.scrim` and inserts it before the text layer in [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:50).
- `AC4` PASS: the card renders the primary genre as the eyebrow and the game title below it through the shared content layer in [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:58) and [GameCard.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/GameCard.tsx:13).
- `AC5` PASS: hover darkening still uses the separate overlay layer above the image, while the text sits on a higher `z-index`; placeholder rendering remains intact in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/patterns/gamePosterCard.ts:14) and [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:37).
- `AC6` PASS: the updated Arrancador delegation test passes and verifies the eyebrow plus wide-image contract in [game-card.test.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/game-card.test.tsx:42).
- `AC7` FAIL: `bunx tsc --noEmit` passes, but renderer build is still blocked by the existing Windows/Bun/Tailwind native binding issue captured in `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/build-renderer.log`.
- `AC8` PASS: no mojibake was introduced in the changed source files; the edited files were re-read after patching and a targeted `rg "�"` check returned no matches.

## Verification
### Fresh commands
- `bunx tsc --noEmit` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` PASS
- `npx vite build` FAIL

### Raw artifacts
- `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/game-card-vitest.log`
- `.agent/tasks/2026-04-19-arrancador-horizontal-game-card/build-renderer.log`

## Notes
- Arrancador already stores `background_image` in the `games` table and exposes it on the frontend model in [database.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/db/database.ts:5) and [index.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/types/index.ts:8), so using horizontal imagery is viable without a schema change.
- RAWG details also expose `background_image_additional` in types, but the current metadata summary/update path only persists `background_image` in [rawg.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/helpers/rawg.ts:24) and [metadata.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/services/metadata.ts:32), so a richer fallback gallery would need a separate follow-up change.
