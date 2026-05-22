# File Search Live Refresh Evidence

Verified on 2026-05-22 after the second File Search feedback pass.

Raw command summary: [`raw/command-results.md`](./raw/command-results.md).

## AC1

Verdict: `PASS`

- `FileSearchResultRow.vue` now keeps the title as the first flexible text
  consumer and lets the path occupy the remaining width.
- Local renderer smoke measured the test title at `462/462` client/scroll
  width, while the long path was truncated at `237/525`; the kind label stayed
  inside the row.

## AC2

Verdict: `PASS`

- Non-empty File Search queries debounce once and then refresh periodically
  until the query changes or the view unmounts.
- Local renderer smoke mocked a first empty `file_index.search` response and a
  second populated response. The file row appeared with no second input or
  launcher show event after two search calls.

## AC3

Verdict: `PASS`

Required verification passed:

- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index`
- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`
- `bun run ark:guard:writes`
- `bun run ark:smoke`
