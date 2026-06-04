# Raycast List.Item.icon

## Goal

Render Raycast `List.Item.icon` in the shell List host without disrupting existing title/subtitle/accessory layout.

## Acceptance Criteria

- Host model preserves `List.Item.icon` as a string image source.
- `RaycastListView.vue` renders a compact fixed-size icon slot when an item has an icon.
- Items without icons retain the current compact layout.
- Accessories continue to align on the right without overlapping text.
- Unit and visual verification cover icon and non-icon list rows.
