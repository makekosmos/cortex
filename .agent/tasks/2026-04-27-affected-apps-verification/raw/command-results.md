# Command Results

## Shared Runtime

```text
> cargo test --manifest-path packages\ark-core\rust\Cargo.toml
PASS
116 library tests passed.
7 ark-core-rpc tests passed.
relay_round_trip passed.
5 sync_round_trip tests passed.
Doc-tests passed.
```

```text
> cargo test --manifest-path services\usage-tracker\Cargo.toml
PASS
4 tests passed.
```

```text
> bun run --cwd packages/kosmos-ark typecheck
PASS
tsc --noEmit
```

## Arrancador

```text
> bun run --cwd apps/arrancador typecheck
PASS
```

```text
> bun run --cwd apps/arrancador test
PASS
47 files passed.
163 tests passed.
```

```text
> bun run --cwd apps/arrancador smoke:packaged
PASS
status: ok
executablePath: D:\Personal\Hobby\Coding\kosmos\apps\arrancador\release\win-unpacked\arrancador.exe
userData: D:\Personal\Hobby\Coding\kosmos\apps\arrancador\.e2e\packaged-smoke\localappdata\arrancador
arkDbPath: D:\Personal\Hobby\Coding\kosmos\apps\arrancador\.e2e\packaged-smoke\ark\ark.db
title: Arrancador
```

## Dashboard

```text
> bun run --cwd apps/dashboard typecheck
PASS
```

```text
> bun run --cwd apps/dashboard test:e2e:smoke
PASS
Smoke dashboard DB seeded at D:\Personal\Hobby\Coding\kosmos\apps\dashboard\.e2e\smoke-dashboard.db
status: ok
statusText: База подключена
topApp: Odyssey Browser
route: sessions
```

## Eden

```text
> bun run --cwd apps/eden/ts test:ark-migration
PASS
{"status":"ok","objectTypes":["note_obj","research_note"],"objects":["note-a","note-b"],"links":["note-a:related:note-b"],"failureStatus":"partial_failure"}
```

```text
> bun run --cwd apps/eden/ts build
PASS
```

```text
> bun run --cwd apps/eden/ts test:e2e
PASS
22 passed.
```

## Delphi

```text
> bun run --cwd apps/delphi/ts test
PASS
8 files passed.
100 tests passed.
```

```text
> bun run --cwd apps/delphi/ts build
PASS
Build completed and packaged with canonical ark-core-rpc sidecar.
```

```text
> bun run --cwd apps/delphi/ts test:e2e -- e2e/shared-ark-task.spec.ts
PASS
1 passed.
```

## Diff Hygiene

```text
> git diff --check
PASS
No whitespace errors.
LF/CRLF normalization warnings only.
```
