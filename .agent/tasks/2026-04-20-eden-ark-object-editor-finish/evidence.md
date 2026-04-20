# Evidence

## Verification Summary

- AC1 `PASS`: `apps/eden/ts/src/components/typed-notes/TypedHeader.vue` now supports `relation` fields, resolves relation ids against `allEntries`, filters hidden fields via `resolveNoteTypeFields()`, and respects `read_only` for inputs and relation chips.
- AC2 `PASS`: `apps/eden/ts/src/Editor.vue` defaults missing note type to `SYSTEM_TYPE_NOTE_ID`, passes `allEntries` and `currentEntryId` into `TypedHeader`, and forwards relation navigation via `@relation-navigate`.
- AC3 `PASS`: `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue` now edits `relation` fields plus `visible`, `read_only`, and `link_type`, hydrates those flags from `ui_schema_json`, and persists `ui_schema_json` through submit.
- AC4 `PASS`: `apps/eden/ts/main/main.ts` imports and calls `shutdownArk()` alongside `shutdownHeart()` on `window-all-closed` and `before-quit`.
- AC5 `PASS`: touched Russian UI strings in edited Eden files are stored as valid UTF-8 text (`Обычная заметка`, `Проверьте поля верхней части заметки`, `Связанные заметки`, etc.).

## Commands

1. TypeScript verification:

```powershell
.\node_modules\.bin\tsc.exe --noEmit
```

Result: `PASS`

2. Targeted oxlint attempt:

```powershell
.\node_modules\.bin\oxlint.exe src/Editor.vue src/components/typed-notes/TypedHeader.vue src/components/settings/ObjectTypesSettings.vue src/lib/typedNotes.ts src/lib/systemTypes.ts main/main.ts
```

Result: blocked by local tool/config loading issue, not by code diagnostics.

Observed error summary: `oxlint` failed to load `apps/eden/ts/vite.config.ts` as ESM config (`ERR_UNKNOWN_FILE_EXTENSION`).
