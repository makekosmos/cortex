# ARK Stage 2 execution

## Definition of done

The dependency graph matches the documented ownership, `ark-store` can be exercised without importing `ark-core`, `ark-net` has no storage dependency, `ark-core` keeps the stable Engine request/response contract, the single-writer and transaction invariants hold, legacy internal facades are deleted with their callers, and the required source, build, test, and GitHub CI checks pass on the final SHA.

## Work units

- [x] Revalidate stage branch and capture independent ownership reviews.
- [x] Remove broad storage access from the transport authorization path and enforce that boundary in the graph gate.
- [ ] Move SQLite query/mutation adapters and schema lifecycle into `ark-store`.
- [ ] Move the single writer executor and snapshot/restore lifecycle into `ark-store`.
- [ ] Move sync session orchestration into `ark-sync` and transport-specific implementations into `ark-net`.
- [ ] Remove obsolete core facades and dependencies while preserving Engine API and wire shapes.
- [ ] Run focused and full checks; commit, push, and verify exact-SHA CI.
- [ ] Update architecture documentation and reconcile KOS-424/KOS-425 evidence in Linear.

## Latest verification

- `pnpm run check:affected` passed on the complete working tree: workspace clippy, 1906 Rust tests, and Manager checks included.
- The pre-commit formatter now resolves the pinned `oxfmt` package directly; its regression test passes when `node_modules/.bin` is absent from `PATH`.

## Decision trail

The detailed interface rationale is in `ark-stage2-boundaries.md`. Run decisions and verification evidence append to `ark-stage2.tsv`.
