# Evidence - Arrancador card text overlay fix

## Scope
- `packages/kosmos-visuals/react/GamePosterCard.tsx`
- `packages/kosmos-visuals/patterns/gamePosterCard.ts`
- `packages/kosmos-visuals/theme/css-variables.css`
- `.agent/tasks/2026-04-19-arrancador-card-text-overlay-fix/spec.md`

## What changed
- Moved all image darkening layers into a dedicated media stack inside [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:38).
- Kept the text content as a separate sibling layer rendered after the media stack in [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/react/GamePosterCard.tsx:63).
- Switched genre/title to plain white in both inline styles and shared CSS.

## Acceptance criteria
- `AC1` PASS: darkening layers now live inside the media layer only.
- `AC2` PASS: the content layer is rendered separately above the media layer.
- `AC3` PASS: genre and title text now use plain white.
- `AC4` PASS: `bunx tsc --noEmit` passes in `apps/arrancador`.
- `AC5` PASS: no mojibake was introduced in changed files.

## Verification
- `bunx tsc --noEmit` PASS
- `rg --line-number "�" .\packages\kosmos-visuals\react\GamePosterCard.tsx .\packages\kosmos-visuals\theme\css-variables.css .\packages\kosmos-visuals\patterns\gamePosterCard.ts .\.agent\tasks\2026-04-19-arrancador-card-text-overlay-fix` PASS
