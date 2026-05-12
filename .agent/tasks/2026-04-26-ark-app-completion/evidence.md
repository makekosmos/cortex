# Evidence

Verification: PASS

## Acceptance Criteria

- AC1 PASS: Arrancador legacy usage backfill no longer writes `sync_kv` through raw SQLite; it uses `@kepler/ark` KV calls backed by `ark-core-rpc`. Game object writes already used ARK object APIs, and source migration DB access is read-only.
- AC2 PASS: Arrancador tests cover game object writes/migration and usage backfill with mocked ARK APIs. `bun run --cwd apps/arrancador test` passed.
- AC3 PASS: Eden note and custom typed-note writes are object-first. Custom note types are ARK object types, note entries are ARK objects, and ARK object listing preserves typed header layout from object type metadata.
- AC4 PASS: Dashboard read-only inspector boundary is documented in `apps/dashboard/AGENTS.md` and `apps/dashboard/README.md`.
- AC5 PASS: `docs/ARK-SMOKE-MATRIX.md` documents a practical smoke matrix using isolated test DBs/temp app data only.
- AC6 PASS: Stale ARK docs were refreshed in `apps/README.md`, `TODO.md`, Eden typed notes docs, Dashboard docs, Arrancador AGENTS, and usage-tracker docs.
- AC7 PASS: Fresh verification was run and recorded here plus `evidence.json` and `raw/command-results.md`.

## Checks

- PASS: `bun run --cwd packages/kepler-ark typecheck`
- PASS: `bun run --cwd apps/arrancador test`
- PASS: `bun run --cwd apps/arrancador typecheck`
- PASS: `bun run --cwd apps/eden/ts test:ark-migration`
- PASS: `bun run build` in `apps/eden/ts`
- PASS: `bunx playwright test tests/app.spec.ts --config playwright.config.ts --grep "custom note type"` in `apps/eden/ts`
- PASS: `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`
- PASS: `cargo test --manifest-path services/usage-tracker/Cargo.toml`
- PASS: Dashboard smoke seed and analytics against `.agent/tasks/2026-04-26-ark-app-completion/smoke/dashboard/smoke-dashboard.db`
- PASS: `git diff --check` with CRLF warnings only

## Notes

`bun run --cwd apps/eden/ts test:e2e -- --grep ...` still loads unrelated spec modules before filtering and hit an old import path from another spec. The verified command targeted `tests/app.spec.ts` directly after a fresh Eden build.
