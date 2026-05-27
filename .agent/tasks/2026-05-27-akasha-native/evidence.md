# Evidence

Verified at: 2026-05-26T23:36:52Z

Raw command notes: [raw/command-results.md](raw/command-results.md)

## AC1 — Existing Vue/static behavior remains compatible

Verdict: PASS.

- `bun run --cwd shell build:extensions` built Arrancador, Delphi, Eden,
  Horologion, and native Akasha.
- `bun run test:e2e -- tests/e2e/extensions-contract.spec.ts` passed all
  five manifest contracts, including existing Vue extensions.

## AC2 — `kind: "native"` manifests are discoverable and open commands launch through `openExtension`

Verdict: PASS.

- `extensions/akasha/manifest.json` declares `kind: "native"` and
  `akasha:open`.
- `bun run test:e2e -- --grep "extension contract: akasha"` passed and verified
  `akasha:open` in `window.kepler.commands.list()`.

## AC3 — Native launch lifecycle and headless behavior

Verdict: PASS.

- `shell/electron/extension-host.ts` tracks native child processes by extension
  id, defaults to single-instance, and passes `--kosmos-extension-id` plus
  `--kosmos-user-data-dir`.
- The Akasha e2e contract passed in headless mode without waiting for or
  spawning a GUI window.

## AC4 — Build/publish tooling handles native extensions

Verdict: PASS.

- `bun run --cwd shell build:extensions` built native Akasha via Cargo.
- `cargo build --release -p akasha` produced the release binary used by native
  `.kext` packaging.
- `node --check shell/scripts/build-extensions.mjs` and
  `node --check shell/scripts/publish-extension.mjs` passed.

## AC5 — Akasha builds as Rust binary and opens GPUI UI

Verdict: PASS.

- `cargo check -p akasha` passed.
- Akasha uses `gpui = 0.2.2` and `gpui-component = 0.5.1` for the window,
  layout, scrollable content, and toolbar buttons.

## AC6 — EPUB fixture parsing and local state persistence

Verdict: PASS.

- `cargo test -p akasha` passed parser and state tests:
  `extracts_spine_text_in_order` and `persists_reader_state`.

## AC7 — Docs and generated agent docs updated

Verdict: PASS.

- Added Akasha app docs and updated extension host / installer docs.
- `bun run docs:build` passed and regenerated `AGENTS.md` / `CLAUDE.md`.
- `bun run docs:check` passed.

## Repository guards

- `bun run ark:guard:writes`: PASS.
- `bun run ark:smoke`: PASS on rerun after initial timeout.
