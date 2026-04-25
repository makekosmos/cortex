# Evidence: Canonical ARK SDK and object-first migration groundwork

## Result

PASS

## Acceptance Criteria

AC1 PASS. New workspace package `packages/kepler-ark` exists with package name `@kepler/ark` and exports the current ARK Node/Electron SDK API.

AC2 PASS. `packages/arksync-node` remains available and re-exports `@kepler/ark`.

AC3 PASS. Electron app SDK imports were rewired to `@kepler/ark` where this pass touches SDK callers.

AC4 PASS. Package manifests and TS/Vite path aliases resolve `@kepler/ark` in Delphi, Arrancador, Dashboard, and the SDK packages.

AC5 PASS. README/TODO/ARK docs name `@kepler/ark` as canonical and describe `@arksync/node` as compatibility-only.

AC6 PASS. TODO/docs record the selected migration direction and out-of-scope/deferred items.

AC7 PASS. Fresh local verification ran against the current codebase. Raw command summaries are in `raw/command-results.md`.

## Verification

- `bun run --cwd packages/kepler-ark typecheck` PASS
- `bun run --cwd packages/kepler-ark build` PASS
- `bun run --cwd packages/arksync-node typecheck` PASS
- `bun run --cwd packages/arksync-node build` PASS
- `bun run --cwd apps/arrancador build:main` PASS
- `bun run --cwd apps/arrancador test` PASS
- `bun run --cwd apps/dashboard typecheck` PASS
- `bun run --cwd apps/dashboard build` PASS
- `bun run --cwd apps/dashboard smoke:seed` PASS
- `bun run --cwd apps/dashboard smoke:analytics` PASS
- `bun run --cwd apps/dashboard test:e2e:smoke` PASS
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `git diff --check` PASS
