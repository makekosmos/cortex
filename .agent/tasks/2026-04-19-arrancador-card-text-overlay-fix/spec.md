# Task: Arrancador card text overlay fix

## Goal

Ensure the game-card darkening affects only the image layer and never visually darkens the genre/title text.

## Acceptance Criteria

- AC1: The shared game card keeps all darkening layers inside the media/image layer.
- AC2: The text content layer is rendered separately above the media layer.
- AC3: Genre and title text use plain white rather than semi-transparent gray-ish white.
- AC4: `bunx tsc --noEmit` passes in `apps/arrancador`.
- AC5: No mojibake is introduced in changed files.
