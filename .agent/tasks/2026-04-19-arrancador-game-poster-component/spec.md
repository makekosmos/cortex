# Task: Arrancador game poster component in kepler-visuals

## Goal
Move the actual React poster-card component into `packages/kepler-visuals` so Arrancador consumes a real shared component rather than recreating the markup locally.

## Acceptance Criteria
- AC1: `kepler-visuals` exports a real React poster-card component, not just a class recipe.
- AC2: Arrancador `GameCard` no longer owns the card markup and instead renders the shared `kepler-visuals` component directly.
- AC3: The shared component supports navigation by path and allows Arrancador to provide its router link implementation.
- AC4: Hover and focus-visible darkening are applied on top of the poster image itself via a `background`-colored overlay at 20% opacity.
- AC5: The current simplified visual treatment remains intact: no visible title, no play button, no hover zoom, no border.
- AC6: Relevant Arrancador tests are updated and pass.
- AC7: `bun run typecheck` passes in `apps/arrancador`.
