# Raw command results

Run date: 2026-04-25

## Delphi

```text
$ bun run --cwd apps/delphi/ts test
Test Files 7 passed (7)
Tests 93 passed (93)
PASS

$ bun run --cwd apps/delphi/ts build:web
tsc + Vite web build
PASS

$ bun run --cwd apps/delphi/ts build
cargo build ark-core-rpc: PASS
tsc: PASS
vite renderer/electron/preload builds: PASS
electron-builder win32 x64 package + NSIS: PASS
PASS

$ node .agent\tasks\2026-04-25-ark-object-first-auto-migrations\raw\delphi-packaged-sidecar-smoke.mjs
binaryPath: apps/delphi/ts/release/win-unpacked/resources/ark-core/ark-core-rpc.exe
listObjectsOk: true
itemCount: 0
PASS

$ Test-Path apps\delphi\ts\sidecar
False
PASS
```

## Eden

```text
$ bun run --cwd apps/eden/ts build
cargo build heart: PASS
cargo build ark-core-rpc: PASS
tsc: PASS
vite renderer/main/preload builds: PASS
PASS
```

## Shared checks

```text
$ cargo test --manifest-path packages\ark-core\rust\Cargo.toml
lib tests: 113 passed
main tests: 7 passed
relay_round_trip: 1 passed
sync_round_trip: 5 passed
doc tests: 0 passed
PASS

$ git diff --check
PASS
Only CRLF normalization warnings were emitted; no whitespace errors.
```
