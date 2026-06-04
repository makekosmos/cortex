# Raycast LocalStorage.allItems

## Goal

Add the Raycast-compatible `LocalStorage.allItems()` API to the private
`@raycast/api` shim and trusted command runtime.

## Acceptance Criteria

- `LocalStorage.allItems()` is exported by `@raycast/api`.
- Memory runtime returns a shallow object containing every local storage key.
- Trusted Raycast command runner storage adapter returns all persisted keys.
- Existing `getItem`, `setItem`, `removeItem`, and `clear` behavior remains unchanged.
- Unit verification covers shim-level and command-runner behavior.
