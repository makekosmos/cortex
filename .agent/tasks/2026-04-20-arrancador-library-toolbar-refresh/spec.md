# Task: Arrancador library toolbar refresh

## Summary
Rework the top controls of the Arrancador library page so the header no longer shows the search field in the top-right area. Replace that space with compact filter and sort controls, and move the view mode toggle into the upper content controls.

## Acceptance Criteria

- AC1: The library page header no longer renders the standalone search input in the top-right control area.
- AC2: The header top-right area renders a filter button with icon and a sort button with icon, both wired to the existing library filtering and sorting behavior.
- AC3: The grid/list view mode toggle is moved from the far-right side of the toolbar into the upper content controls while preserving the existing view switching behavior.
- AC4: Existing library interactions still compile and the affected library test coverage passes against the updated UI.
