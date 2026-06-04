# Raycast Command Mode Validation

## Goal

Raycast package manifest parsing should reject explicit unknown `commands[].mode` values instead of silently treating them as `view`.

## Acceptance Criteria

- Missing `mode` still defaults to `view`.
- Explicit valid modes remain `view`, `no-view`, and `menu-bar`.
- Explicit unknown non-empty modes cause that command to be skipped.
- Packages with only invalid commands are rejected.
- Unit tests cover default, valid, invalid, and mixed command lists.
- Docs and evidence are updated.
