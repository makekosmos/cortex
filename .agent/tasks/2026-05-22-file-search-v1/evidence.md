# File Search v1 Evidence

Verified on 2026-05-22 against the working tree after implementation.

Raw command summary: [`raw/command-results.md`](./raw/command-results.md).

## AC1

Verdict: `PASS`

- `services/kepler-backend/src/file_index/` defines the host-local scanner,
  filesystem watcher, store, ranking, settings, and `file-index.db`
  initialization.
- `services/kepler-backend/src/main.rs` creates the file index beside backend
  instance data and starts a background scan.
- `services/kepler-backend/src/ws_server.rs` exposes `file_index.search`,
  `file_index.open`, `file_index.rescan`, and settings operations.
- Verification command: `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`.

## AC2

Verdict: `PASS`

- Focused backend tests prove default noisy-folder exclusion and persisted
  inclusion mode using temp roots only.
- Verification command:
  `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index`.

## AC3

Verdict: `PASS`

- `shell/src/views/LauncherView.vue` debounces non-empty file queries through
  `file_index.search`, renders file-labeled hits with path subtitles, and opens
  chosen files through `file_index.open`.
- Verification commands:
  `bun run --cwd shell typecheck` and `bun run --cwd shell build:js`.

## AC4

Verdict: `PASS`

- Settings general tab exposes the Russian-language noisy-folder exclusion
  toggle. The toggle calls `file_index.settings_set`; backend persists the
  setting in `file-index.db` and rescans with the new mode.
- Verification commands:
  `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index`
  and `bun run --cwd shell typecheck`.

## AC5

Verdict: `PASS`

Required verification commands passed:

- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`
- `bun run ark:guard:writes`
- `bun run ark:smoke`
