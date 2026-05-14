# Evidence - Arrancador game detail poster removal

## Scope
- `apps/arrancador/src/pages/GameDetail.tsx`
- `.agent/tasks/2026-04-19-arrancador-game-detail-poster-removal/spec.md`

## What changed
- Removed the vertical `cover_image` poster block from the main game detail header area in [GameDetail.tsx](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/GameDetail.tsx:890).
- Kept the game title/genre/actions layout and left the edit dialog cover-image controls intact.

## Acceptance criteria
- `AC1` PASS: the game detail page no longer renders the vertical poster in the hero/title section.
- `AC2` PASS: the title, genre, and play/actions layout remains intact after poster removal.
- `AC3` PASS: the edit dialog still retains `cover_image` fields and preview controls.
- `AC4` PASS: `bunx tsc --noEmit` passes in `apps/arrancador`.
- `AC5` PASS: no mojibake was introduced in changed files.

## Verification
- `bunx tsc --noEmit` PASS
- `rg --line-number "�" .\apps\arrancador\src\pages\GameDetail.tsx .\.agent\tasks\2026-04-19-arrancador-game-detail-poster-removal` PASS
