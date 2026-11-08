# Manager

The Engine's control panel: a Rust/GPUI host that runs TSX pages, the same stack
Atlas uses. The pages own no truth — the Engine is the only source of facts, and
every page either shows what a call answered or says it has nothing to show.

```
manager/
  src/            the host: discovery, lifetime, RPC, the bridge to the pages
    boot.rs       start/stop the Engine the way its supervisor expects
    engine.rs     engine.lock.json → /v1/health, /v1/info, /v1/rpc; the poller
    data.rs       which calls a page costs, and how an answer lands in the state
    bridge.rs     `manager.bridge`: commands in, state and navigate out
    identity.rs   the crate's names: product name, bundle id, client class
  ui/             the pages (TSX), and `bundle.js` — their compiled form
  assets/         the design's typeface
```

## Working on it

```sh
pnpm run check:manager                 # fmt + clippy + tests, at the repo root
cargo test --manifest-path manager/Cargo.toml
cargo test --manifest-path manager/Cargo.toml -- --ignored   # needs a running Engine

cd manager/ui
nsh check .                            # type check the pages
nsh test .capture/Screens.test.tsx --target windows          # every page, as PNG
nsh build . && cp .nsh/out/bundle.js bundle.js               # refresh the bundle
```

Where the window would move from — `HTCAPTION` for the titlebar and the
sidebar, `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE` for the caption buttons — is
answered by Windows from hitboxes, so it can be asked without touching the
mouse: `pwsh manager/scripts/window-hit-test.ps1 -Exe manager/target/debug/manager.exe`.
It sends the move the hit test keys off and reads back the code per probe point.

`ui/bundle.js` is committed on purpose: `src/main.rs` embeds it with
`include_str!`, so a fresh clone — and the release build — must find it without
an `nsh` toolchain. Rebuild it whenever a page changes; the screenshot tests
catch a page that drifted from the design, not a stale bundle.

## Running it against a dev Engine

```sh
MUNDUS_DATA_DIR=<dir with engine.lock.json> cargo run --manifest-path manager/Cargo.toml
MUNDUS_SKIP_ENGINE_BOOT=1   # leave the Engine's lifetime to whoever started it
MUNDUS_MANAGER_OFFSCREEN=1  # park the window off the desktop (soak runs)
MUNDUS_NSH_DEV_SERVER=ws://127.0.0.1:8790/ws   # hot reload from `nsh dev`
```

## What it is not

- It does not talk to the OS, the database, the microphone or the network on its
  own. Every one of those is an Engine operation.
- The Apps page lists nothing, because this Engine keeps no installed-app
  catalog; the Start-Menu index is the system's, not Mundus's.
- It carries no Run-at-login of its own, no browser of its own, and no update
  download without a press.
