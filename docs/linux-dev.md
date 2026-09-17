# Linux development: host + Engine + first-party apps

Verified on Ubuntu 24.04 x86_64 (KOS-53, KOS-93). The supported gate is the
first-party E2E suite, which builds the Engine, spawns the pinned
`ark-core-rpc` sidecar, installs the signed Agenda/Memoria `.kspkg`, and
launches the Electron Host under Xvfb — all in isolated `/tmp` roots with
PID-identity cleanup.

## Toolchain

```text
rustup toolchain install 1.95.0          # workspace requires >= 1.88
sudo apt-get install build-essential pkg-config libssl-dev xvfb
node --version                           # Node 20+
corepack prepare pnpm@12.4.1 --activate  # pinned packageManager
```

`pkg-config` + `libssl-dev` are required by `openssl-sys` in the Engine.
`xvfb` provides the display; `KOSMOS_HEADLESS=1` keeps windows hidden in tests.

## Engine + ARK sidecar

```text
cargo build -p kepler-backend            # writes target/debug/kepler-backend
cargo build --bin ark-core-rpc           # writes target/debug/ark-core-rpc
```

The Engine discovers the sidecar via `ARK_CORE_RPC_PATH` or workspace
candidates; the E2E harness passes an explicit path. Manual run:

```text
KOSMOS_DATA_DIR=/tmp/kosmos-data KOSMOS_HEADLESS=1 target/debug/kepler-backend &
cat /tmp/kosmos-data/engine.lock.json    # { pid, http_port, auth_token }
curl -H "Authorization: Bearer <auth_token>" http://127.0.0.1:<http_port>/v1/...
```

Package manifests declare `targets[].os`; the Engine maps non-Windows/non-macOS
to `linux`, so a first-party manifest with `"os": ["windows", "linux"]` is
accepted without host changes.

## Host dependencies

`host/package.json` depends on `@makekosmos/ark` from GitHub Packages, so the
normal path needs a token with `read:packages`:

```text
NODE_AUTH_TOKEN=<token with read:packages> pnpm --dir host install --frozen-lockfile
```

If the token cannot read private packages, fall back to the pinned workspace
checkout (`node scripts/workspace.mjs bootstrap` provisions
`.tmp/workspace/arca-sdk`), temporarily pointing the dependency at it with
`link:.tmp/workspace/arca-sdk` — do not commit that edit.

```text
pnpm --dir host run typecheck
pnpm --dir host run build                # writes host/dist-electron
```

## E2E (headless)

```text
xvfb-run -a pnpm --dir host run e2e \
  first-party-agenda-contract.spec.ts first-party-agenda-smoke.spec.ts \
  first-party-memoria-contract.spec.ts first-party-memoria-smoke.spec.ts \
  first-party-memoria-import-crash.spec.ts
```

- `first-party-agenda-contract` — signed Agenda installs, launches in Host,
  performs scoped ARK reads/writes through `window.kosmosApp.ark`, denies
  out-of-grant operations, and survives an Engine restart with persisted data.
- `first-party-agenda-smoke` — the daily smoke: packaged Agenda UI capture
  ("Новая задача") → row in Входящие (Inbox) → task card, with the object
  verified through the ARK bridge.
- `first-party-memoria-contract` — signed Memoria installs, rolls back a
  partial Obsidian-vault import, performs scoped ARK CRUD through
  `window.kosmosApp.ark`, denies out-of-grant operations, and survives an
  Engine crash + restart with persisted data.
- `first-party-memoria-smoke` — the daily smoke: packaged Memoria UI
  "Добавить заметку" → title + body typed in the Tiptap editor → autosave →
  card in Всё (Everything), with the note verified through the ARK bridge.
- `first-party-memoria-import-crash` — an import interrupted after a durable
  entry write rolls back on the next Engine/Host launch.

The harness is platform-neutral: binary names come from `executableName()`,
fixture ZIPs use `desktop/scripts/zip-utils.mjs`, and process cleanup reads
`/proc` on Linux (PID + start-time identity) while Windows keeps the
PowerShell/WMI path. Linux Electron containers typically need `--no-sandbox`,
which the specs add only on `process.platform === "linux"`.

## Intentional gaps

- `pnpm install` for `host/` requires `read:packages` on GitHub Packages;
  without it use the `link:` fallback above.
- Specs for other first-party apps (Arcadia/Daedalus/Dictation/Ordo/Shell/
  topology) assume the monorepo layout or Windows-only tools and are
  not part of the Linux gate.
- Tray, global shortcuts, dictation capture, and start-menu integration
  remain Windows-only; the Engine stubs them out off-Windows.
