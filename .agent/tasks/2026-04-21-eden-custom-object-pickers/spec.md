# Eden Custom Object Property Pickers

## Summary

Replace native `select` controls in Eden typed object properties with app-native custom pickers inspired by Anytype, covering both single-select and multi-select object fields.

## Acceptance Criteria

- AC1: `select` and `multi_select` typed object fields no longer render native browser/OS `select` controls in the object header editor.
- AC2: A shared custom picker component supports both single-choice and multi-choice property fields with the same visual language.
- AC3: `play_status` uses the custom single-select picker and `genres` uses the custom multi-select picker in game objects.
- AC4: The picker opens as an in-app floating panel/popover, not as a platform-native dropdown.
- AC5: The field row layout remains full-width for game objects: one property per line, label on the left, value area on the right.
- AC6: Verification includes code-level checks for the removed native `select` usage in typed property fields and a UTF-8/mojibake pass on touched UI strings.
