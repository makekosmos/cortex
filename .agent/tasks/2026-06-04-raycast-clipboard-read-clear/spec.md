# Raycast Clipboard read/readText/clear

## Goal

Add low-risk Clipboard read and clear compatibility to the private
`@raycast/api` shim and trusted command runtime.

## Acceptance Criteria

- `Clipboard.readText()` returns text from the configured runtime clipboard.
- `Clipboard.read()` returns the same text-only value for the current
  compatibility slice.
- `Clipboard.clear()` clears the configured clipboard.
- Trusted command runner bridges `readText` and `clear` through the host
  clipboard adapter.
- Existing `Clipboard.copy` and `Clipboard.paste` behavior remains unchanged.
- Unit verification covers shim-level and trusted command-runner behavior.
