# Generic Object Model v1

## Goal

Introduce a generic Ark-backed object layer with built-in `note_obj` and `game_obj`, then wire Eden and Arrancador to that layer so objects have typed visible/hidden properties, note bodies, and canonical game records backed by Ark.

## Scope

- `packages/ark-core/rust`
- `apps/eden/ts/main`
- `apps/eden/ts/src`
- `apps/arrancador/electron/main`
- `apps/arrancador/src`

## Acceptance Criteria

- AC1: Ark schema includes generic `objects`, `object_types`, and `object_links` tables, plus CRUD/load operations exposed via `ark-core-rpc`.
- AC2: Ark seeds built-in `note_obj` and `game_obj` type definitions with `title` as a required top-level field and type-level `visible_fields`, `hidden_fields`, and `read_only_fields` metadata.
- AC3: Eden can list, load, create, and save Ark-backed objects as note entries, using `note_obj`/`game_obj` type metadata to render visible properties above the editor body.
- AC4: Eden object type editing supports field visibility, read-only flags, and a `relation` field kind, and persists those definitions to Ark.
- AC5: `related_notes` for `note_obj` and `game_obj` are stored canonically in `object_links` and editable from Eden.
- AC6: Arrancador games are linked to Ark via a local `ark_object_id`, and add/update/read flows upsert/read a canonical `game_obj`.
- AC7: `game_obj` usage fields (`total_playtime_seconds`, `last_played_at`, `play_count`) are hydrated from Ark usage tables and exposed as read-only properties in Eden/Arrancador.
- AC8: Touched Eden UI strings for this feature remain valid UTF-8 with no new mojibake in the object type or typed-object flows.

## Constraints

- Do not revert unrelated dirty worktree changes.
- Keep the existing Eden editor body format in `content_json`.
- Keep Ark usage tables as the source of raw usage data; `game_obj` only aggregates/project them.
- Prefer minimal, additive migration steps over rewriting the existing Eden `heart` store from scratch.
