# ARK Read-Only SQL Boundary

There are two separate questions:

1. Who is allowed to write ARK data?
2. Who is allowed to inspect ARK data?

The write rule is strict: app TypeScript services must not write directly to ARK
tables such as `objects`, `object_types`, `object_links`, `tracked_apps`,
`usage_sessions`, `usage_events`, or `sync_kv`. Writes go through
`ark-core-rpc` / `@kepler/ark`, or through Rust `ark_core::db` helpers in
dedicated Rust writers such as `usage-tracker`.

Read-only inspection is looser:

- Dashboard is a read-only inspector/analytics app. It may inspect a selected
  ARK SQLite database from Electron main, similar to a database console.
- Arrancador normal app flows should read through `@kepler/ark` first. Direct
  read-only SQLite is allowed only as an emergency fallback when the ARK runtime
  is unavailable.
- Renderer code must not open SQLite directly.

If the project later wants an absolutely strict API-only model for reads too,
the next step is to replace the remaining fallback SQLite paths with dedicated
ARK runtime endpoints. Some of those endpoints now exist:

- `objects.listByType` -> `list_objects_by_type`
- `objects.getMany` -> `get_objects_by_ids`
- `usage.processes.recent` -> `list_recent_usage_processes`
- `usage.processes.search` -> `search_usage_processes`
- `usage.gamePlaytime.summary` -> `get_usage_game_playtime_summary`

Still useful future endpoints:

- app-specific bulk analytics queries that avoid loading full usage snapshots

Until then, direct read-only SQL is allowed only in Electron main services,
must stay visibly separate from write paths, and must not be used by automated
tests against a user/main database.
