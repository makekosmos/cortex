# Raycast List.Item Accessories

## Goal

Render basic Raycast `List.Item` accessories in the List host so ported commands can show right-side status/value chips.

## Acceptance Criteria

- `List.Item` accessories are normalized from serializable `props.accessories`.
- String accessories and object accessories with `text`, `title`, `tag`, or `date` are shown.
- Accessories render on the right side of list rows without changing row height unexpectedly.
- Empty/unsupported accessories are ignored.
- Unit and visual verification cover the accessory rendering path.
