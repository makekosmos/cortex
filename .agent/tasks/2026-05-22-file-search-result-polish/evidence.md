# File Search Result Polish Evidence

Verified on 2026-05-22 after the feedback fix.

Raw command summary: [`raw/command-results.md`](./raw/command-results.md).

## AC1

Verdict: `PASS`

- `file_index::display_name` falls back from blank stored names to the path
  filename before serializing search results.
- Focused backend tests cover a blank stored-name row.

## AC2

Verdict: `PASS`

- File Search now sends a larger candidate window into the Rust ranker before
  returning the visible result limit.
- Focused backend tests cover a word-prefix match surviving many shorter infix
  filename matches.

## AC3

Verdict: `PASS`

- `FileSearchResultRow.vue` owns document-style file icons, basename/title
  fallback presentation, and the fixed grid columns for truncating long paths.
- A local Playwright smoke against the shell renderer confirmed an empty API
  name displays `fallback-file.json`, and a long path had
  `clientWidth < scrollWidth` inside the ellipsized path column.

## AC4

Verdict: `PASS`

Required verification passed:

- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index`
- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`
- `bun run ark:guard:writes`
- `bun run ark:smoke`
