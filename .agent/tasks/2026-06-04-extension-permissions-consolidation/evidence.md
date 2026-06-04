# Evidence — 2026-06-04 extension permissions + consolidation pass

## Subagents

- 4 subagents were used:
  - Security explorer for extension permission model and bypass search.
  - Docs drift worker for stale root/docs-site references and docs guard coverage.
  - Artifact/repro worker for `.agent` runtime artifacts, Cargo.lock, Bun version.
  - Triage explorer for stale vs actionable review findings.

## Fixed

- P0 extension permissions:
  - User-installed extensions are denied by default.
  - Manifest capabilities are enforced for ARK requests, ARK event subscriptions, userData IPC, and focus widget state.
  - Trusted first-party status is source-based (`dev` / `bundled`), not id-based.
  - `params.operation` is rejected before forwarding.
- P1 headless guards:
  - Re-opening existing dashboard/settings/install windows no longer calls visible focus/show paths in headless/test mode.
- P1 sync identity:
  - `ArkClient.invokeOperation()` normalizes local write `device_id` to the client slot identity for all raw write operations.
  - Spoofed raw `device_id` cannot become the HLC suffix in `lan_sync.version_vector`.
- P1 Delphi legacy runtime:
  - Removed runtime SpaceSetup/settings spaces UI.
  - Removed fake `KEPLERDEFAULT` / fake spaces / `lan-sync:*` no-op bridge behavior.
  - Removed local JSON/local DB/P2P protocol files from Delphi runtime graph.
  - Delphi Electron bootstrap now loads tasks directly from single ARK DB.
- Docs drift/repo hygiene:
  - Root docs and docs-site references were updated by the docs worker.
  - `.agent` runtime artifacts are ignored/removed from index.
  - Root `Cargo.lock` generated for the binary Rust workspace.
  - Bun `packageManager` aligned to `bun@1.3.14`.
  - Tracked Android `libark_core.so` is documented as intentional APK native runtime, not an accidental artifact.
  - `@kosmos/ark` is documented as a source-only workspace package.
  - Oversized tracked PNG icon sources were resized to 1024×1024 and icon size policy was documented.
  - Delphi Zed Mono font asset source/license policy was documented.
- Akasha EPUB guardrails:
  - Oversized source files, excessive ZIP entries, excessive uncompressed entry/total sizes, oversized covers, and excessive reader blocks now fail fast.
- ARK storage hardening:
  - Legacy Delphi, usage, pending-object, and sync_kv upserts no longer use SQLite `INSERT OR REPLACE`; they update rows in-place with `ON CONFLICT DO UPDATE`.
  - Stale backend comment that described `upsert_object_type` as `INSERT OR REPLACE` was corrected.

## Verification Run So Far

- `bun test tests/unit/extension-permissions.test.ts` — PASS.
- `bun test tests/unit/ark-client-invoke-device-id.test.ts` — PASS.
- `bun run shell:typecheck` — PASS.
- `bun run shell:build` — PASS.
- `bun test tests/unit/akasha-epub-guardrails.test.ts` — PASS.
- `bunx playwright test tests/e2e/extensions-contract.spec.ts --grep akasha` — PASS.
- `bunx playwright test tests/e2e/extension-permissions.spec.ts` — PASS.
- `bunx playwright test tests/e2e/delphi-legacy-cleanup.spec.ts` — PASS.
- `bunx playwright test tests/e2e/extension-permissions.spec.ts tests/e2e/headless-window-repeat-open.spec.ts tests/e2e/delphi-legacy-cleanup.spec.ts tests/e2e/extensions-contract.spec.ts tests/e2e/commands-architecture.spec.ts` — PASS, 15/15.
- `bunx playwright test tests/e2e/headless-window-repeat-open.spec.ts` — PASS (earlier in loop).
- `bunx playwright test tests/e2e/extensions-contract.spec.ts` — PASS (earlier in loop).
- `bunx playwright test tests/e2e/commands-architecture.spec.ts` — PASS (earlier in loop).
- `cargo metadata --locked` — PASS (artifact worker).
- `cargo test -p ark-core` — PASS (166 core unit tests + RPC tests + proptest + relay/sync integration).
- `rg -n "INSERT OR REPLACE|REPLACE INTO" crates/ark-core/rust/src services/kepler-backend/src` — PASS, no production SQL matches.
- `bun run docs:sync` — PASS.
- `bun run docs:check` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run --cwd packages/ark typecheck` — PASS.
- `bun run ark:smoke` — PASS after stopping stale repo dev/Vite processes that had locked `shell/dist/index.html`; first run had passed Rust/backend stages and failed only on Windows EPERM during Vite out-dir cleanup.
- `git diff --check` — PASS (Windows CRLF warnings only).

## Visual Evidence

- `.tmp/visual/2026-06-04-delphi-legacy-cleanup/delphi-main-no-space-setup.png`
- Resized `shell/build/icon.png` and `extensions/eden/icon.png` visually inspected after downscaling to 1024×1024.
