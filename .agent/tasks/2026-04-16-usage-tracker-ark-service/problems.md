# Problems

No acceptance-criterion blocker remains for this task. The following issues were observed during verification but are outside the usage-tracker extraction scope.

- `bun run test` in `apps/arrancador` still fails in existing UI/provider tests:
  - `src/test/ui-primitives.test.tsx`: `missing sheet-close`
  - `src/test/ui-sidebar.test.tsx`: `missing sidebar-trigger`
  - `src/test/ui-sidebar.test.tsx`: mobile sidebar assertion failure
- `bun run check:rust` in `apps/arrancador` still fails because `cargo fmt --check` reports existing formatting drift under `src-tauri/**`.
- `services/usage-tracker/src/config.rs`, `db.rs`, `model.rs`, `sampler.rs`, `singleton.rs`, and `tracker.rs` are currently dead scaffolding next to the active `main.rs` implementation.
- `apps/arrancador/` is untracked in the root repo after nested git removal. This affects repository hygiene/staging, not runtime behavior.
