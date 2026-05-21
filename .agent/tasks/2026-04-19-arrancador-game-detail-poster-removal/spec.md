# Task: Arrancador game detail poster removal

## Goal

Remove the vertical cover/poster from the main game detail page while keeping the rest of the page and edit flows intact.

## Acceptance Criteria

- AC1: The game detail page no longer renders the vertical `cover_image` poster in the hero/title section.
- AC2: The game title, genre, and play/actions layout remain intact after removing the poster block.
- AC3: The edit dialog still retains `cover_image` fields and preview controls.
- AC4: `bunx tsc --noEmit` passes in `apps/arrancador`.
- AC5: No mojibake is introduced in changed files.
