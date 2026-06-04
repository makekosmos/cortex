# Spec - Raycast List.Dropdown host slice

## Classification

FULL_LOOP. This is a Raycast UI compatibility slice inside the shell host and
touches renderer state, callback IPC, and compatibility docs.

## Goal

Trusted Raycast-compatible `List` commands can render a first
`List.Dropdown` search-bar accessory in the Kosmos Raycast host, including
grouped dropdown items and an optional trusted `onChange` callback.

## Acceptance Criteria

- `ListProps` exposes `searchBarAccessory`.
- The Raycast view normalizer serializes `searchBarAccessory` as host-renderable
  children.
- `List.Dropdown` `onChange` callbacks are registered through the existing
  trusted callback registry.
- The Vue List host renders the dropdown next to the search input without
  layout overlap.
- Choosing an option invokes the guarded Raycast session action IPC when a
  callback exists.
- Docs mention the supported boundary.

## Out of Scope

- Raycast-style async search throttling.
- Extension-controlled dropdown value reconciliation after re-render.
- Dropdown-driven filtering semantics beyond local selection state.
- Keyboard shortcut parity.
