# File Search Fast Path Evidence

Verified on 2026-05-23.

Raw command summary: [`raw/command-results.md`](./raw/command-results.md).

## AC1

Verdict: `PASS`

- `LauncherView.vue` tracks pending File Search requests and renders
  `Ищем файлы...` with a spinner while the active query request is pending.
- An isolated headless renderer smoke awaited that label during a live
  `file_index.search` request, then awaited the resulting file row.

## AC2

Verdict: `PASS`

- `FileStore` now keeps a trigram FTS5 table in sync with the host-local files
  table and uses it for filename/path candidate search for queries with at least
  three characters.
- Short queries keep the existing `LIKE` fallback.
- Focused backend tests cover FTS filename and path substring matches alongside
  the existing File Search result/ranking coverage.

## AC3

Verdict: `PASS`

- Windows fixed drive roots now attempt `FSCTL_QUERY_USN_JOURNAL` followed by
  `FSCTL_ENUM_USN_DATA` enumeration, reconstruct file paths from USN V2 parent
  file references, and keep noisy-folder filtering.
- If opening/enumerating the NTFS volume fails, scanner logging records the
  fallback and the existing `WalkDir` root scan continues.
- Focused NTFS coverage proves parent-FRN path reconstruction and noisy-folder
  filtering; the scanner remains compiled by the full Windows backend suite.

## AC4

Verdict: `PASS`

Required verification passed:

- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index`
- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`
- `bun run ark:guard:writes`
- `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\target-ark-smoke-file-search'; bun run ark:smoke`
