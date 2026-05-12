# Apps

Kepler apps are product shells around the shared ARK runtime. The current
desktop integration path is:

1. Renderer displays UI and calls a narrow preload API.
2. Electron main owns app orchestration.
3. Electron main talks to `ark-core-rpc` through `@kepler/ark`.
4. ARK owns shared SQLite schema, object storage, usage storage, search, and sync.

Direct writes into ARK SQLite tables are not an app integration path. If an app
needs to write shared data, add or use an ARK runtime operation and call it
through `@kepler/ark`.

## App Boundaries

- `delphi`: task UI over ARK `task_obj` objects.
- `eden`: note/editor UI over ARK `note_obj` and custom object types; Heart can
  remain for editor/vault-specific behavior and legacy migration.
- `arrancador`: game UI over ARK `game_obj` objects and ARK usage data.
- `dashboard`: read-only ARK database inspector/analytics UI. It may inspect a
  selected ARK SQLite database from Electron main, but it must not write.

See `docs/ARK-READONLY-SQL-BOUNDARY.md` for the distinction between allowed
read-only inspection and forbidden direct app writes.

## Testing

All automated checks must use isolated databases or temp app-data paths. Do not
point tests, smoke checks, migration verification, or Playwright runs at a main
user ARK database.

The root smoke matrix is available as:

```powershell
bun run ark:smoke
```
