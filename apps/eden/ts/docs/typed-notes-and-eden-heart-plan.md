# Typed Notes and Eden Heart Plan

Eden notes are ARK objects.

Current source-of-truth rules:

- Default notes use ARK object type `note_obj`.
- Custom typed notes use their custom ARK object type id.
- Note types are saved as ARK `object_types`.
- Note entries are saved as ARK `objects`.
- Related-note relationships are saved as ARK `object_links`.
- Startup migration copies legacy Heart note types and entries into ARK object
  types/objects and writes a backup/report for the migration pass.

Heart remains available for editor/vault-specific behavior, one-time migration
source reads, import/export, and future specialized Rust search/indexing work.
It is not the permanent source of truth for shared note identity/state after
ARK object migration.

Normal note/type/search reads now use ARK objects and object types. Heart reads
of entries/types are migration inputs, not runtime fallback.
It should not be the write target for new typed note entries.

See also `docs/EDEN-HEART-ARK-BOUNDARY.md`.

## Data Mapping

`Entry` to ARK object:

- `entry.id` -> `objects.id`
- `entry.type_id ?? "note_obj"` -> `objects.type_id`
- `entry.title` -> `objects.title`
- `entry.content_json` -> `objects.content_json`
- `entry.header_props_json` -> `objects.props_json`
- `entry.deleted_at` -> `objects.deleted_at`

`NoteType` to ARK object type:

- `noteType.id` -> `object_types.id`
- `noteType.name` -> `object_types.name`
- `noteType.schema_json` -> `object_types.schema_json`
- `noteType.ui_schema_json` plus presentation fields -> `object_types.ui_schema_json`

## Testing

Typed-note tests must use a temporary vault/app-data path. Do not point Eden
tests at a main user ARK database.
