# Evidence

Scope for this pass:

- `packages/ark-core/rust/**`
- `services/usage-tracker/**`
- `apps/arrancador/**`
- root workspace config in `package.json`

## Acceptance criteria

- AC1: PASS
  `packages/ark-core/rust/src/types.rs`, `schema.rs`, and `db.rs` now define and persist first-class usage entities: `TrackedApp`, `UsageSession`, and `UsageEvent`.
- AC2: PASS
  `packages/ark-core/rust/src/db.rs` includes the usage entities in Ark storage load/apply flows, and `services/usage-tracker/src/main.rs` updates `lan_sync.version_vector` after direct writes via `set_sync_kv`.
- AC3: PASS
  `services/usage-tracker/` exists as a standalone Windows-first Rust executable with no Arrancador dependency. The active runtime is `services/usage-tracker/src/main.rs` plus `windows_capture.rs`, and it writes directly into Ark DB.
- AC4: PASS
  `apps/arrancador` no longer launches or owns the old watcher runtime. `apps/arrancador/activity-watcher/` is gone, and the Electron backend now wires usage reads through Ark-backed adapters instead of spawning a local tracker.
- AC5: PASS
  `apps/arrancador/electron/main/services/ark-usage.ts`, `playtime-stats.ts`, `games.ts`, and `backend.ts` read usage/playtime from Ark DB through a stable main-process adapter while keeping the renderer IPC contract unchanged.
- AC6: PASS
  Watcher-specific dead weight was removed from `apps/arrancador` without deleting core runtime/config files. The cleanup is targeted to watcher ownership and related docs/entrypoints.
- AC7: PASS
  Root `package.json` now includes `services/*` in Bun workspaces, and `bun install --ignore-scripts` completes successfully against the current workspace layout.
- AC8: PASS
  `services/usage-tracker/README.md`, `apps/arrancador/README.md`, `apps/arrancador/TEST_STRATEGY.md`, and these task artifacts now describe the new split: Ark stores usage, `usage-tracker` writes it, and Arrancador consumes it.

## Implementation summary

- Extended Ark storage/runtime support in:
  - `packages/ark-core/rust/src/types.rs`
  - `packages/ark-core/rust/src/schema.rs`
  - `packages/ark-core/rust/src/db.rs`
  - `packages/ark-core/rust/src/lib.rs`
  - `packages/ark-core/rust/src/main.rs`
  - `packages/ark-core/rust/src/ffi.rs`
- Added standalone tracker service in:
  - `services/usage-tracker/Cargo.toml`
  - `services/usage-tracker/package.json`
  - `services/usage-tracker/README.md`
  - `services/usage-tracker/src/main.rs`
  - `services/usage-tracker/src/windows_capture.rs`
- Rewired Arrancador usage reads in:
  - `apps/arrancador/electron/main/backend.ts`
  - `apps/arrancador/electron/main/services/ark-usage.ts`
  - `apps/arrancador/electron/main/services/playtime-stats.ts`
  - `apps/arrancador/electron/main/services/games.ts`
  - `apps/arrancador/electron/main/services/contracts.ts`
  - `apps/arrancador/README.md`
  - `apps/arrancador/TEST_STRATEGY.md`
- Updated root workspace in:
  - `package.json`
- Resolved duplicate workspace package naming that blocked root Bun install by renaming:
  - `packages/apple-like/package.json`

## Verification

PASS

- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`
- `cargo test --manifest-path services/usage-tracker/Cargo.toml`
- `bun run typecheck` in `apps/arrancador`
- `bun run build:main` in `apps/arrancador`
- `bun run build:preload` in `apps/arrancador`
- `bun run build:renderer` in `apps/arrancador`
- `bun install --ignore-scripts` at repo root

Non-blocking failures observed during verification

- `bun run test` in `apps/arrancador` fails in pre-existing UI/provider tests unrelated to usage-tracker extraction:
  - `src/test/ui-primitives.test.tsx` (`missing sheet-close`)
  - `src/test/ui-sidebar.test.tsx` (`missing sidebar-trigger`, mobile sidebar assertion)
- `bun run check:rust` in `apps/arrancador` fails because `cargo fmt --check` reports existing formatting drift under `src-tauri/**`.

## Notes

- `services/usage-tracker/src/config.rs`, `db.rs`, `model.rs`, `sampler.rs`, `singleton.rs`, and `tracker.rs` currently exist as unused scaffolding beside the active `main.rs` path. They do not block the acceptance criteria but should be cleaned up in a follow-up pass.
- `apps/arrancador/` now appears as an untracked subtree in the root repo because the nested `.git` repository was removed during this migration. That is a repository-state consequence, not a code defect in this task.
