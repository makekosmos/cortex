# Task: Arrancador horizontal card render fix

## Goal
Restore visible game cards in Arrancador after the horizontal card redesign by removing fragile dependencies on external-package utility classes for critical card geometry.

## Acceptance Criteria
- AC1: Shared game cards remain visibly rendered in Arrancador after the horizontal layout change.
- AC2: The card height no longer depends on an arbitrary Tailwind `aspect-*` utility defined only inside the external `kepler-visuals` package.
- AC3: The bottom scrim and title layer still render correctly after the fix.
- AC4: Relevant Arrancador verification still passes.
- AC5: No mojibake is introduced in changed files.
