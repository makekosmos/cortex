# Raycast Form Static Fields

## Goal

The Raycast-compatible form host should support static form content: `Form.Description` text and `Form.Separator` dividers.

## Acceptance Criteria

- `@raycast/api` shim exports `Form.Description`.
- The form model keeps `Form.Description` and `Form.Separator` in field order without requiring user input values.
- `RaycastFormView` renders descriptions and separators with Kosmos visuals tokens.
- Existing form value collection and submit behavior remain unchanged for input fields.
- Unit tests and visual verification cover description + separator + submit.
- Docs and evidence are updated.
