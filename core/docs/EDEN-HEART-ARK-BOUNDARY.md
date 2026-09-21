# Eden Heart / ARK Boundary

Eden Heart is still useful and should not be removed just because Eden notes are
ARK objects.

The intended split is:

- ARK owns shared data identity and syncable state: `object_types`, `objects`,
  and `object_links`.
- Eden Heart owns heavy editor/vault-local work: vault import/export,
  one-time migration source reads, editor-oriented transforms, and future
  specialized search/indexing work if ARK search is not enough.
- New Eden note writes go to ARK objects. Heart is not a permanent fallback for
  shared note identity/state after startup migration.

This keeps the original idea intact: complex work can live in Rust, while the
shared application data model stays in ARK.

Search decision:

- ARK search means searching `objects.title`, `objects.content_json`, and
  `objects.props_json` for data that is already stored as ARK objects.
- Heart search means a future Eden-specific Rust index for editor/vault content
  if ARK object search is not rich enough.
- Current default: shared notes are searchable through ARK; Heart can still be
  used later for a richer Eden-only index without becoming the source of truth.

Current runtime rule:

- `loadEntry`, `listEntries`, `listNoteTypes`, and `searchEntries` read ARK
  objects/object types only.
- Heart entry/type reads are used by startup migration, not by normal shared
  note read paths.
