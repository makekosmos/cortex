# Raycast ActionPanel.Submenu

## Goal

Add a basic Raycast-compatible `ActionPanel.Submenu` so action panels can group secondary actions without losing executable child actions or shortcuts.

## Acceptance Criteria

- `packages/raycast-api` exposes `ActionPanel.Submenu`.
- The host model preserves submenu entries for rendering.
- `actionNodes(...)` flattens submenu leaf actions for shortcut dispatch and callback execution.
- `RaycastActionPanel.vue` renders a submenu trigger and a compact popover of child actions.
- Clicking a submenu child emits the executable child action and closes the submenu.
- Unit and visual verification cover serialization, model flattening, and UI behavior.
