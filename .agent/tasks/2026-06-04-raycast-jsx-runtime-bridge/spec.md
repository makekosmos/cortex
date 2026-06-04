# Raycast JSX Runtime Bridge

## Goal

Support trusted compiled TSX commands that import `@raycast/api/jsx-runtime`
through the Raycast command runner bridge.

## Acceptance Criteria

- The command runner rewrites `@raycast/api/jsx-runtime` imports to a local
  bridge file.
- The JSX runtime bridge exports `jsx`, `jsxs`, and `Fragment`.
- Existing `@raycast/api` bridge behavior remains unchanged.
- A trusted view command fixture using `@raycast/api/jsx-runtime` returns a
  host-renderable snapshot.
- Unit verification covers the compiled TSX-style import path.
