# Raycast Keyboard.Shortcut.Common

## Goal

Expose a minimal Raycast-compatible `Keyboard.Shortcut.Common` object so extensions can reference common action shortcuts through `@raycast/api`.

## Acceptance Criteria

- `@raycast/api` exports `Keyboard`.
- `Keyboard.Shortcut.Common` includes useful common shortcuts for Copy/Open/Save/Close.
- Common shortcuts serialize through existing Action `shortcut` props.
- Host `actionShortcut(...)` produces expected labels/codes for common shortcuts.
- Unit verification covers export shape and host shortcut normalization.
