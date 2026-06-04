# Raycast Action Shortcuts

## Goal

Raycast-compatible `Action` shortcut props should be visible in the Kosmos Raycast action panel and executable through renderer keyboard events.

## Acceptance Criteria

- `Action` / specialized actions preserve `shortcut` props in snapshots.
- `RaycastActionPanel` renders a compact shortcut label next to actions that define a shortcut.
- Keyboard dispatch uses `KeyboardEvent.code` for letter keys so RU layout does not break shortcuts.
- Supported modifiers include `cmd`, `ctrl`, `shift`, `alt`, and common aliases.
- Unit tests cover shortcut serialization and matching helper behavior.
- Visual verification captures a shortcut label and executes the action through keyboard input.
- Docs and evidence are updated.
