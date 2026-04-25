# Evidence: ARK object migration regression tests

## Result

PASS

## Acceptance Criteria

AC1 PASS. Delphi migration coverage was added in `apps/delphi/ts/src/services/storage/task-object-migration.test.ts` for legacy todo migration into `task_obj`.

AC2 PASS. Delphi tests cover idempotency, object-first `loadAll`, single upsert, and batch upsert through dependency-injected helpers in `apps/delphi/ts/shared/task-object-migration.ts`.

AC3 PASS. Eden migration coverage was added in `apps/eden/ts/scripts/testArkObjectMigration.ts` and the reusable migration helper lives in `apps/eden/ts/main/ark-object-migration.ts`.

AC4 PASS. The Eden test asserts link writes happen only after both source and target ARK objects exist.

AC5 PASS. `apps/delphi/AGENTS.md` no longer contains `delphi-db`, `@arksync/node`, or the old `packages/arksync-node/src/ark-client.ts` guidance.

AC6 PASS. Fresh verification ran against the current codebase. Raw command summaries are in `raw/command-results.md`.

## Verification

- `bun run --cwd apps/delphi/ts test` PASS
- `bun run --cwd apps/delphi/ts build:web` PASS
- `bun run --cwd apps/eden/ts test:ark-migration` PASS
- `bun run --cwd apps/eden/ts build` PASS
- `rg -n "delphi-db|@arksync/node|packages/arksync-node/src/ark-client.ts" apps\delphi\AGENTS.md` PASS with no matches
- `git diff --check` PASS
