# Task: Arrancador shared game card via kosmos-visuals

## Goal
Move Arrancador's reusable game card visual into `kosmos-visuals`, then consume that shared primitive from Arrancador with the updated simplified visual treatment.

## Acceptance Criteria
- AC1: Arrancador game-card visual recipe is implemented in `kosmos-visuals` as a shared primitive that Arrancador imports instead of owning the visual styling locally.
- AC2: Arrancador keeps only a thin wrapper around the shared primitive, passing the destination path and cover/placeholder content needed for game cards.
- AC3: The card no longer renders the visible game title, no longer shows the centered play button overlay, no longer zooms on hover, and no longer has a border.
- AC4: Hover treatment is reduced to a subtle darkening only.
- AC5: Relevant Arrancador tests are updated for the new shared-card behavior.
- AC6: `bun run typecheck` and `bun run test` pass in `apps/arrancador`.
