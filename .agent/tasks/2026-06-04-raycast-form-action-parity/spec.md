# Raycast Form Action Parity

## Goal

Bring Form action execution closer to List/Grid/Detail by supporting common action types from a Form footer.

## Acceptance Criteria

- `RaycastFormView` executes `Action.CopyToClipboard`, `Action.OpenInBrowser`, and `Action.Open` through guarded session IPC.
- Existing `Action.Paste`, `Action.ShowInFinder`, `Action.Trash`, `Action.LaunchCommand`, generic `Action`, and `Action.SubmitForm` behavior remains intact.
- `Action.Push` from a Form footer opens the pushed `Detail` target locally.
- Unit and visual verification cover the expanded Form action behavior.
- Docs and evidence are updated.
