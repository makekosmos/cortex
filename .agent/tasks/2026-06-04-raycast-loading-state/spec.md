# Spec - Raycast List/Grid loading state slice

## Classification

FULL_LOOP. This extends Raycast root component rendering in the shell host and
requires visual verification.

## Goal

Trusted Raycast-compatible `List` and `Grid` view commands can expose
`isLoading: true`, and the Kosmos Raycast host renders a loading state instead
of an empty state.

## Acceptance Criteria

- `listIsLoading()` extracts `List.props.isLoading`.
- `gridIsLoading()` extracts `Grid.props.isLoading`.
- `RaycastListView.vue` renders `Загрузка...` while loading and suppresses
  `List.EmptyView` text when there are no visible items.
- `RaycastGridView.vue` renders `Загрузка...` while loading and suppresses
  `Grid.EmptyView` text when there are no visible items.
- Unit tests cover List/Grid loading extraction.
- Visual verification covers a loading List state.
- Docs mention the supported boundary.

## Out of Scope

- Animated spinner parity.
- Loading overlays with partial stale results.
- Async `onSearchTextChange` lifecycle and throttling.
