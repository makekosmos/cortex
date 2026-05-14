# Task: Arrancador horizontal game card

## Goal
Switch Arrancador's shared game card from a portrait poster to a horizontal media card and render basic metadata directly on top of the image.

## Acceptance Criteria
- AC1: The shared `kosmos-visuals` game card renders in a horizontal aspect ratio suitable for landscape artwork instead of the current portrait poster ratio.
- AC2: Arrancador prefers a game's wide/background image for the card and falls back to the existing cover image only when a wide image is unavailable.
- AC3: The card always renders a bottom darkening gradient above the image so overlaid text remains readable.
- AC4: The card renders the primary genre in regular text at the lower-left area, with the game title directly below it in a heavier weight.
- AC5: Existing hover darkening still works above the image, does not hide the text layer, and the placeholder state remains functional when no image exists.
- AC6: Relevant Arrancador tests are updated for the shared card contract and pass.
- AC7: `bun run typecheck` and `bun run build:renderer` pass in `apps/arrancador`.
- AC8: No mojibake is introduced in the changed source files.
