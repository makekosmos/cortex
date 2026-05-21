# Verification Summary

Status: PASS

## Commands

- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`
  - Result: PASS
  - Raw: [raw/ark-core-cargo-test.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/object-model-v1/raw/ark-core-cargo-test.txt)
- `apps/eden/ts/node_modules/.bin/tsc.exe --noEmit -p apps/eden/ts/tsconfig.json`
  - Result: PASS
  - Raw: [raw/eden-tsc.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/object-model-v1/raw/eden-tsc.txt)
- `apps/arrancador/node_modules/.bin/tsc.exe --noEmit -p apps/arrancador/tsconfig.json`
  - Result: PASS
  - Raw: [raw/arrancador-tsc.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/object-model-v1/raw/arrancador-tsc.txt)
- `rg -n "Р[А-Яа-я]" apps/eden/ts/src/Editor.vue apps/eden/ts/src/components/settings/ObjectTypesSettings.vue apps/eden/ts/src/lib/systemTypes.ts apps/eden/ts/src/components/typed-notes/TypedHeader.vue apps/eden/ts/main/main.ts apps/eden/ts/main/store.ts`
  - Result: PASS, no matches
  - Raw: [raw/utf8-mojibake-check.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/object-model-v1/raw/utf8-mojibake-check.txt)

## Acceptance Criteria

- AC1 PASS: Ark schema and RPC now include `objects`, `object_types`, and `object_links`, with CRUD/load support in [schema.rs](/D:/Personal/Hobby/Coding/kosmos/packages/ark-core/rust/src/schema.rs:113), [db.rs](/D:/Personal/Hobby/Coding/kosmos/packages/ark-core/rust/src/db.rs:462), and [main.rs](/D:/Personal/Hobby/Coding/kosmos/packages/ark-core/rust/src/main.rs:71).
- AC2 PASS: built-in `note_obj` and `game_obj` are seeded in Ark with schema and UI metadata in [db.rs](/D:/Personal/Hobby/Coding/kosmos/packages/ark-core/rust/src/db.rs:32) and surfaced in Eden system definitions in [systemTypes.ts](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/lib/systemTypes.ts:1).
- AC3 PASS: Eden lists, loads, saves, and deletes Ark-backed entries via [store.ts](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/main/store.ts:271), and renders object properties above the editor body through [Editor.vue](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/Editor.vue:14) and [TypedHeader.vue](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/components/typed-notes/TypedHeader.vue:1).
- AC4 PASS: Eden object-type editing supports `relation`, `visible`, and `read_only`, and persists `ui_schema_json` in [ObjectTypesSettings.vue](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/components/settings/ObjectTypesSettings.vue:181).
- AC5 PASS: `related_notes` are projected to and from canonical `object_links` in [store.ts](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/main/store.ts:311) and edited via the relation UI in [TypedHeader.vue](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/components/typed-notes/TypedHeader.vue:92).
- AC6 PASS: Arrancador stores local `ark_object_id` in [database.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/db/database.ts:8), reads/writes it in [games.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/services/games.ts:16), and syncs canonical `game_obj` records through [ark-game-objects.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/services/ark-game-objects.ts:335).
- AC7 PASS: Arrancador usage hydration reads Ark usage tables in [ark-usage.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/services/ark-usage.ts:131), and `game_obj` usage props are written/read as read-only fields in [systemTypes.ts](/D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/lib/systemTypes.ts:99) and [ark-game-objects.ts](/D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/services/ark-game-objects.ts:300).
- AC8 PASS: touched Eden object-model files passed the mojibake grep with no suspicious `Р...` sequences in source text.

## Residual Risks

- Existing Arrancador rows are not bulk-backfilled with `ark_object_id`; linkage becomes persisted after later Arrancador writes.
- Arrancador game deletion does not yet soft-delete the linked Ark object, so orphaned `game_obj` rows remain possible.
- Relation UI is currently a generic multi-select over all entries and does not yet constrain choices by target type.
