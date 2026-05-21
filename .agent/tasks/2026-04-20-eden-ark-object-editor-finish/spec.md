# Eden Ark Object Editor Finish

## Goal

Finish the remaining Eden-side renderer/editor work for Ark-backed object types so typed headers can handle relation fields, object-type editing can persist Ark visibility and read-only metadata, the editor defaults new notes to `note_obj`, and Eden shuts Ark down cleanly on app exit.

## Scope

- `apps/eden/ts/src/Editor.vue`
- `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
- `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
- `apps/eden/ts/src/store/eden.ts`
- `apps/eden/ts/src/lib/typedNotes.ts`
- `apps/eden/ts/src/lib/systemTypes.ts`
- `apps/eden/ts/main/main.ts`
- `apps/eden/ts/main/store.ts`
- `apps/eden/ts/src/vite-env.d.ts`

## Acceptance Criteria

- AC1: `TypedHeader` renders Ark relation fields in a usable way, respects visible/read-only metadata for field rendering, and keeps existing field kinds working.
- AC2: `Editor.vue` passes the typed-header context needed for relation rendering, defaults untyped/new Eden entries to `note_obj`, and does not regress autosave/validation.
- AC3: `ObjectTypesSettings.vue` can edit and persist `relation`, `visible`, `read_only`, and `ui_schema_json`-backed data for note types without removing existing type editor behavior.
- AC4: Eden main-process shutdown closes Ark alongside Heart if Ark is running.
- AC5: Touched Russian UI strings in the modified Eden flow are valid UTF-8 and do not introduce new mojibake.

## Constraints

- Change only Eden files.
- Do not revert unrelated worktree changes.
- Prefer minimal additive edits over refactors.
