# Delphi Project Status Bridge

## Original Task Statement

User report summary:

- creating or updating a project fails in `db:upsertProject`
- runtime error: `invalid type: integer 0, expected a string`

## Summary

Fix the Delphi Electron <-> Rust sidecar bridge for project persistence.

The current Delphi app uses numeric `ProjectStatus` enum values in application state, while the Rust sidecar expects `project.status` as a string in JSON. The bridge must convert between these representations so project creation, update, load, and local persistence all work consistently.

## Acceptance Criteria

- AC1: `dbUpsertProject(...)` no longer sends numeric `status` values to the Rust sidecar.
- AC2: `dbLoadAll()` converts persisted sidecar project statuses back into Delphi's numeric `ProjectStatus` enum shape.
- AC3: Existing Delphi store/UI code can continue to treat `project.status` as numeric without further changes.
- AC4: Delphi TypeScript compile check passes after the fix.
- AC5: Delphi production build passes after the fix.

## Constraints

- Keep the diff focused on the sidecar bridge layer.
- Do not redesign the project domain model in this task.
- Prefer a narrow compatibility mapper over broad refactors.

## Non-Goals

- Ark project status model redesign.
- Rust schema migration.
- Sidebar or project UI redesign.

## Verification Plan

1. Run Delphi TypeScript compile check.
2. Run Delphi production build.
3. Inspect bridge code to confirm:
   - outgoing `Project.status` is serialized as string
   - incoming sidecar `Project.status` is normalized to numeric enum values
