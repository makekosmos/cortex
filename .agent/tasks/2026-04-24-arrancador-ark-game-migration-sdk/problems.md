# Verification Problems

## P1: Biome import ordering

The first focused Biome check failed because the new test's `@arksync/node` type imports were not sorted. Apply the safe import-order fix.

## P2: Vitest `bun --cwd` startup EPERM

The first focused Vitest command used `bun --cwd apps/arrancador vitest ...` and failed while Vite loaded config with `spawn EPERM`. Rerunning from `workdir=apps/arrancador` hit the same Vite startup issue before tests executed. Added `verify-ark-game-migration.ts`, a focused Bun verifier that imports the migration service and runs the same SDK-injected scenarios without Vite.

## P3: Standalone verifier exposed Electron import coupling

The first standalone verifier failed because `ark-game-migration.ts` statically imported `ark-game-objects`, which imports `electron.app`. Bun outside Electron cannot load that named export. The smallest fix was to lazy-import `getArkCoreRpcBinaryPath` only when the migration creates its own `ArkClient`; SDK-injected tests no longer depend on Electron runtime.
