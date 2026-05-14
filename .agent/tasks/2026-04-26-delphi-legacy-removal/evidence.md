# Evidence: Remove Delphi legacy sidecar after ARK migration

Result: PASS

## Acceptance Criteria

- AC1 PASS: Delphi scripts now use `build:ark` / `build:ark:dev` and build `packages/ark-core/rust --bin ark-core-rpc`; no Delphi package path builds or copies a legacy Delphi DB sidecar.
- AC2 PASS: Delphi task load flow remains ARK object-first and no longer falls back to legacy todos after migration.
- AC3 PASS: `apps/delphi/ts/sidecar` is not present/tracked, and docs mark it as removed rather than a fallback.
- AC4 PASS: Eden normal shared note entry/type/search reads now use ARK objects/object types/search only; Heart remains for migration/export/folder/vault-specific operations.
- AC5 PASS: Docs/TODO state `task_obj` ARK objects are the Delphi source of truth after startup migration.
- AC6 PASS: Fresh verification passed with isolated test/smoke databases.

## Verification

- PASS: `bun run --cwd apps/delphi/ts test`
- PASS: `bun run --cwd apps/delphi/ts build`
- PASS: `bun run --cwd apps/eden/ts test:ark-migration`
- PASS: `bun run --cwd apps/eden/ts build`
- PASS: `bun run --cwd packages/kosmos-ark typecheck`
- PASS: `bun run ark:smoke`
- PASS: `git diff --check` with line-ending warnings only

## Notes

Two sandbox runs hit process-spawn restrictions (`spawn EPERM`) before rerun with escalation:

- Delphi Vitest config loading.
- Delphi Electron Builder `app-builder.exe`.

Both commands passed when rerun with spawn permission.
