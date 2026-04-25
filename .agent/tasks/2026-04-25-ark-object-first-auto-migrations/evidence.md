# Evidence: ARK object-first automatic migrations

## Result

PASS

## Acceptance Criteria

AC1 PASS. Delphi no longer builds, packages, or falls back to `delphi-db`; `build:sidecar` builds `packages/ark-core/rust --bin ark-core-rpc`, packaged resources contain `ark-core-rpc.exe`, and `apps/delphi/ts/sidecar` is removed.

AC2 PASS. Delphi migrates legacy todos to `task_obj` records on app entry and after `db:switchSpace`.

AC3 PASS. Delphi `db:loadAll`, `db:upsertTodo`, `db:deleteTodo`, `db:batchUpsertTodos`, and ARK task IPC paths now use ARK object helpers as the primary task model.

AC4 PASS. Eden `initStore` migrates Heart note types and entries into ARK object types and ARK note objects.

AC5 PASS. Eden migration creates all note objects before syncing note links.

AC6 PASS. The replacement is documented in Delphi AGENTS override and task artifacts: `ark-core-rpc` owns ARK SQLite `ark.db` and generic objects replace the removed `delphi-db` path.

AC7 PASS. Fresh verification ran against the current codebase. Raw command summaries are in `raw/command-results.md`; packaged Delphi sidecar smoke script is in `raw/delphi-packaged-sidecar-smoke.mjs`.

## Verification

- `bun run --cwd apps/delphi/ts test` PASS
- `bun run --cwd apps/delphi/ts build:web` PASS
- `bun run --cwd apps/delphi/ts build` PASS
- Packaged `ark-core-rpc.exe` smoke `init` + `list_objects` PASS
- `Test-Path apps\delphi\ts\sidecar` returned `False`
- `bun run --cwd apps/eden/ts build` PASS
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `git diff --check` PASS
