# ARK Smoke Matrix

Use this matrix for ARK app integration checks. Every command must run against
an isolated test database, `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/`, or an OS
temp directory. Never use a main/user ARK database for automated checks.

The root smoke command runs the current automated matrix:

```powershell
bun run ark:smoke
```

Set a task-local root first:

```powershell
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\2026-04-26-ark-app-completion\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null
```

## Core

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
bun run --cwd packages/kosmos-ark typecheck
```

## Usage Tracker

```powershell
cargo test --manifest-path services\usage-tracker\Cargo.toml
```

For manual smoke runs, pass an explicit DB path under the smoke root:

```powershell
$env:ARK_DB_PATH = "$env:KOSMOS_SMOKE_ROOT\usage-tracker\ark.db"
```

## Dashboard

```powershell
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
node --experimental-strip-types apps\dashboard\scripts\smokeAnalytics.ts --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
bun run --cwd apps/dashboard test:e2e:smoke
```

## Delphi / Eden Shared Objects

```powershell
bun run --cwd apps/delphi/ts test:e2e -- e2e/shared-ark-task.spec.ts
bun run --cwd apps/eden/ts test:ark-migration
```

These checks create their own temporary app-data/vault paths and must continue
to avoid main user databases.

## Arrancador

```powershell
bun run --cwd apps/arrancador test
bun run --cwd apps/arrancador smoke:packaged
```

`smoke:packaged` builds an unpacked Windows bundle with signing/editing disabled
for smoke only, launches `release/win-unpacked/arrancador.exe`, and passes
temporary `APPDATA`, `LOCALAPPDATA`, and `ARK_DB_PATH` paths under
`apps/arrancador/.e2e/packaged-smoke`.

Do not run migration/backfill scripts against a real ARK DB during smoke
verification. Pass explicit source and target paths under the smoke root when a
manual migration check is needed.
