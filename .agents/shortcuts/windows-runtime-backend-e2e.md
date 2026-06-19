# Windows Runtime Backend E2E

## Trigger

When a desktop e2e test needs a newly added `platform/runtime` operation, but the
renderer build is fresh and Electron still reports an old/unknown backend op.

## Symptom

`window.kepler.ark.request("dictation.some_new_op")` or another runtime op fails with
`unknown sub-operation`, even after `vite build`; or `target/debug/kepler-backend.exe`
is locked by a stale Electron/backend process.

## Do This

Build a fresh backend binary and point Electron at it explicitly:

```powershell
$env:CARGO_TARGET_DIR = ".tmp\cargo-dictation"
cargo build --manifest-path Cargo.toml --bin kepler-backend
$env:KEPLER_BACKEND_EXE = (Resolve-Path .tmp\cargo-dictation\debug\kepler-backend.exe).Path
rtk npx playwright test --config platform/desktop/playwright.config.ts <spec> --grep "<test name>"
```

If the test waits for full Ark readiness and the isolated backend cannot find the data
engine, also set `ARK_CORE_RPC_PATH` to an existing `target\debug\ark-core-rpc.exe` or
`target\release\ark-core-rpc.exe`.

Expected timings from 2026-06-19:

- `rtk bunx vite build --config vite.config.mjs` in `platform/desktop`: about 1m22s
  renderer build plus about 5s Electron bundles.
- isolated cold `kepler-backend` build with `CARGO_TARGET_DIR=.tmp\cargo-dictation`:
  about 5m10s.
- targeted AI settings e2e with cold backend startup: about 33s.

## Avoid

Do not rely on `platform/desktop` `test:e2e` to rebuild Rust runtime; it rebuilds JS.
Do not keep polling tiny 10-20s timeouts for cold Rust builds. If `target/debug` is
locked, stop stale `kepler-backend.exe` processes or use an isolated `CARGO_TARGET_DIR`.

## Promote To Skill When

Promote this if more desktop/runtime e2e workflows start needing explicit backend/data
engine build orchestration.
