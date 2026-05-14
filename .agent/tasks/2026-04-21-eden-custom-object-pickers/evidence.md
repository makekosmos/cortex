# Evidence

## Result

Verification status: `PASS`

The typed object property editor now uses an app-native picker for single-choice and multi-choice fields instead of native platform `<select>` controls. The implementation is shared across `select`, `multi_select`, and editable `relation` fields, normalizes stale Ark/Arrancador values before validation, and keeps the table rows stable while allowing multi-select content to grow vertically.

## Acceptance Criteria

- `AC1` `PASS` — `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue` no longer renders native `<select>` in typed property fields. Static verification is recorded in `raw/no-native-select.txt`.
- `AC2` `PASS` — shared picker extracted to `apps/eden/ts/src/components/typed-notes/ObjectPropertyPicker.vue` and wired from `ObjectPropertyField.vue`.
- `AC3` `PASS` — game `play_status` continues to use single-select, and `genres` is now defined as `multi_select` with options in `apps/eden/ts/src/lib/systemTypes.ts`.
- `AC4` `PASS` — picker opens in an in-app floating panel/popover teleported to `body`, with fixed positioning and high z-index, so it no longer gets clipped or hidden under header content.
- `AC5` `PASS` — header row layout remains full-width and two-column for game objects; label and value share the same text size, rows keep a uniform minimum height, multi-select values wrap downward as needed, and read-only fields are marked with a lock glyph.
- `AC6` `PASS` — verification includes static no-native-select checks plus a mojibake/UTF-8 pass for newly added UI strings. Results are recorded in `raw/no-native-select.txt` and `raw/ui-strings-utf8.txt`.

## Checks

- `PASS` — `apps/eden/ts/node_modules/.bin/tsc.exe --noEmit -p "D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json"`  
  Raw: `raw/tsc.txt`
- `PASS` — `bun run lint` in `apps/eden/ts`  
  Raw: `raw/lint.txt`
- `PASS` — `rg -n "<select" "D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\src\components\typed-notes"`  
  Raw: `raw/no-native-select.txt`
- `PASS` — `Select-String` UTF-8/mojibake pass for new Russian UI strings  
  Raw: `raw/ui-strings-utf8.txt`
- `PASS` — overlay/table layout static checks for teleport/z-index/width sync and uniform row rhythm  
  Raw: `raw/picker-overlay-and-table-layout.txt`

## Files

- `apps/eden/ts/src/components/typed-notes/ObjectPropertyPicker.vue`
- `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`
- `apps/eden/ts/src/lib/systemTypes.ts`
- `apps/eden/ts/src/lib/typedNotes.ts`

## Notes

- Legacy saved game headers that stored `genres` as a comma-separated string are normalized to arrays, and nullable Ark/Arrancador values such as `rawg_id: null` are coerced into schema-safe defaults before validation.
- Legacy built-in game note types are upgraded through `normalizeSystemNoteType()` now that it refreshes `schema_json` as well as presentation JSON, so stale stored schemas no longer keep `genres` as a plain text field.
- The dropdown panel now uses `Teleport` + `position: fixed` and derives width from the trigger rect, matching the Anytype-style overlay behavior more closely than the earlier inline absolute panel.
- Multi-select values render as wrapped tokens inside the value cell, so the row can expand downward instead of clipping or collapsing back to a single line.
- Arrancador reads and writes `play_status` and `genres` through Ark object props in `apps/arrancador/electron/main/services/ark-game-objects.ts`; static evidence is recorded in `raw/arrancador-sync.txt`.
- The repository has unrelated pre-existing worktree changes outside this task; they were not modified or reverted here.
