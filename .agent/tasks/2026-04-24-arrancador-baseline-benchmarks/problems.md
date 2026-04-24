# Problems and Fixes

## P1: raw artifact directory was missing

- Symptom: first verification commands could not redirect output into `.agent/tasks/2026-04-24-arrancador-baseline-benchmarks/raw/`.
- Fix: created the required `raw/` directory before rerunning commands.

## P2: scan iterator cleanup was not runtime-neutral

- Symptom: `bun run bench:baseline` failed in executable scan with `TypeError: undefined is not an object (evaluating 'dir.close().catch')`.
- Cause: `dir.close()` can return `undefined` in the active runtime path, while the code assumed a Promise.
- Fix: changed scan cleanup to `await Promise.resolve(dir.close()).catch(() => undefined)`.

## P3: direct Bun execution cannot load `better-sqlite3`

- Symptom: running the benchmark directly through Bun failed with `'better-sqlite3' is not yet supported in Bun`.
- Fix: `bench/run-baseline.mjs` now bundles the TypeScript benchmark and runs it through Electron in Node mode.

## P4: regular Node ABI did not match Electron native module ABI

- Symptom: running the bundled benchmark with normal Node failed because `better_sqlite3.node` was compiled against Electron's module ABI.
- Fix: the launcher sets `ELECTRON_RUN_AS_NODE=1` and invokes the local Electron binary.

## P5: benchmark exceptions could keep the process alive

- Symptom: after an exception, the event-loop monitor interval kept the process alive until command timeout.
- Fix: the monitor interval is `unref()`'d and `measured()` stops it in `catch`.

## P6: repeated numbered SQLite placeholders were incorrectly normalized

- Symptom: `games.searchGames()` failed with `RangeError: Too few parameter values were provided`.
- Cause: SQLite adapter replaced `?1` with `?`, turning `name LIKE ?1 OR exe_name LIKE ?1` into two positional placeholders while still passing one parameter.
- Fix: added `normalizeNumberedPlaceholders()` to duplicate numbered parameters by placeholder occurrence and covered it with a unit test.

## P7: generated benchmark bundle polluted Biome checks

- Symptom: `bun run biome:check` inspected `apps/arrancador/.tmp/bench/run-baseline.mjs`.
- Fix: added `.tmp` to `apps/arrancador/.gitignore`; Biome is configured to respect the ignore file.
