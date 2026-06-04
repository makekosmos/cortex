# Raycast Form Dropdown Sections

## Goal

`Form.Dropdown.Section` must preserve grouped options in the Kosmos Raycast form host instead of flattening all items into one ungrouped select.

## Acceptance Criteria

- The form model exposes dropdown option sections with titles while keeping a flat options list for existing callers.
- `RaycastFormView` renders grouped dropdown options as native `optgroup` blocks.
- Ungrouped dropdown items still render as plain options.
- Unit tests cover mixed root dropdown items and titled sections.
- Visual verification captures a form dropdown with a visible section and successful submit.
- Docs and evidence are updated.
