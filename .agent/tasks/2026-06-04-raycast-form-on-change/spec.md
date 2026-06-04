# Raycast Form Field OnChange

## Goal

Raycast-compatible `Form.*` field `onChange` callbacks must be invoked by the Kosmos Raycast form host when users edit field values.

## Acceptance Criteria

- `normalizeRaycastNode()` already preserves field `onChange` callback ids; the form model exposes them per field.
- Text, textarea, password/date/dropdown, checkbox, and file picker value changes call guarded Raycast session IPC with `{ value }`.
- Fields without callbacks keep existing local-only behavior.
- Static `Form.Description` / `Form.Separator` never emit change callbacks.
- Unit tests cover callback id extraction and callback registration.
- Visual verification changes a dropdown, receives a callback through the mocked IPC, and submits the updated value.
- Docs and evidence are updated.
