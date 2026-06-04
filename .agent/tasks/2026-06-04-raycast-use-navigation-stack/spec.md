# Raycast useNavigation Stack

## Goal

Support Raycast `useNavigation().push/pop/popToRoot` for trusted view commands by maintaining a host-owned navigation stack per Raycast session.

## Acceptance Criteria

- Runtime adapter forwards `navigationPush`, `navigationPop`, and `navigationPopToRoot` from trusted commands.
- The Electron Raycast view host owns a stack of normalized snapshot roots per session.
- Navigation updates publish a guarded snapshot update to the renderer session window.
- Renderer root view listens for snapshot updates and swaps the displayed root.
- Existing local `Action.Push` behavior remains compatible.
- Unit tests cover runtime forwarding and snapshot normalization paths.
- Docs and evidence are updated.
