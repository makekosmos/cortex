# Raycast Detail Actions

## Goal

Raycast-compatible `Detail` roots must preserve and render their `actions` prop so markdown/detail-only commands can expose footer actions through the existing Kosmos Raycast host.

## Acceptance Criteria

- `Detail({ actions: ActionPanel(...) })` normalizes `ActionPanel` as a snapshot child alongside `Detail.Metadata`.
- The host model exposes root `Detail` action panels without affecting list item detail panes.
- Root `Detail` views render actions in a footer and execute supported action types through guarded Raycast session IPC.
- The behavior is covered by unit tests and a visual verification screenshot.
- Docs and proof evidence reflect the new compatibility surface.
