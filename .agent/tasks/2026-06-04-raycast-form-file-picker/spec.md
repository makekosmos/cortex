# Spec - Raycast Form.FilePicker host slice

## Classification

FULL_LOOP. This extends the Raycast form host across shared IPC, preload,
Electron main-process dialog handling, Vue renderer UI, tests, and docs.

## Goal

Trusted Raycast-compatible form commands can render `Form.FilePicker`, open a
native file picker through a guarded Raycast session IPC call, and submit the
selected paths as `string[]` form values.

## Acceptance Criteria

- `@raycast/api` exposes a typed `Form.FilePicker` component shape.
- `formModel()` returns `Form.FilePicker` fields with `string[]` defaults and
  file picker option flags.
- `window.kepler.raycast.pickFiles(sessionId, request)` exists in shared
  preload types.
- Electron main handles `kepler:raycast:pick-files` only for the matching
  Raycast session window.
- `RaycastFormView.vue` renders a FilePicker field with selected path rows and
  updates form values from the picker result.
- Unit tests cover snapshot normalization and form model extraction.
- Visual verification covers the rendered picker after mocked file selection.
- Docs mention the supported boundary.

## Out of Scope

- Drag-and-drop file selection.
- File type filters and extension validation.
- Persisted form drafts.
- Untrusted Raycast JS sandboxing and capability prompts.
