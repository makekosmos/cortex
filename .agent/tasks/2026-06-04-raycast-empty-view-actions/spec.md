# Raycast EmptyView.actions

## Goal

Render Raycast `List.EmptyView.actions` and `Grid.EmptyView.actions` in the host footer when the current list/grid result set is empty.

## Acceptance Criteria

- `normalizeRaycastNode(...)` preserves `actions` passed to `List.EmptyView` and `Grid.EmptyView`.
- Host model exposes empty-state action panels for List and Grid.
- `RaycastListView.vue` uses empty-state actions when no visible items remain and the view is not loading.
- `RaycastGridView.vue` uses empty-state actions when no visible items remain and the view is not loading.
- Item actions continue to take precedence when an item is selected.
- Unit and visual verification cover List/Grid empty actions.
