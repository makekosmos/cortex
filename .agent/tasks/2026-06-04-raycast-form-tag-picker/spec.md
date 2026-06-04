# Raycast Form.TagPicker

## Goal

Add a Raycast-compatible `Form.TagPicker` host component with selectable tag values, submit integration, and field-level `onChange`.

## Acceptance Criteria

- `packages/raycast-api` exposes `Form.TagPicker` and `Form.TagPicker.Item`.
- The host model reads tag picker options from child items.
- `defaultValue` / `value` are normalized as `string[]`.
- The Vue form host renders selected tags and available tags using Kosmos visuals tokens.
- Selecting or removing a tag updates form values and invokes field-level `onChange` with `string[]`.
- Form submit includes selected tag values.
- Unit tests and visual verification cover the new field.
