# AGENTS.md — `core/` subtree

`core/` is the vendored subtree of the old `makekosmos/core` repo, merged into
cortex intentionally (KOS-129) so the Engine and its data runtime ship as one
application. It now contains only:

- `crates/kernel/` — the shared Rust + SQLite runtime ("ARK"), hosted
  in-process by the Engine via `kernel::service::ArkService`
  (`../runtime/src/ark_host.rs`). Local rules and invariants:
  [`crates/kernel/AGENTS.md`](crates/kernel/AGENTS.md).
- `integrations/fatsecret/` — ARK type registration data
  (`type-registration.json`).

The retired `makekosmos/docs` repository is no longer the doc source.
Cross-cutting ARK topics are documented in-tree at the repo root:
`docs/sync.md`, `docs/write-boundary.md`.

All root rules apply (`/AGENTS.md`): the never-rules, the pnpm gate
(`pnpm run check:affected` / `check`), the brand gate, one logical change per
commit.
