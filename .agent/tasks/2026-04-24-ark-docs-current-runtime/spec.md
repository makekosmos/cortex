# Task: Current ARK Runtime Documentation

## Context

Root docs still point to historical `packages/ark/` and older `packages/arksync/` architecture. Current ARK lives in `packages/ark-core/rust`, is consumed through `ark-core-rpc`, and the TypeScript integration path is `@arksync/node`.

## Scope

- Update the root README ARK link/summary to point at `packages/ark-core/README.md`.
- Add a current `packages/ark-core/README.md` describing the real Rust runtime, SQLite schema, sidecar protocol, SDK path, sync status, packaging expectations, and direct-write policy.
- Mark legacy TODO/work-progress documents as historical so future work does not follow obsolete `packages/ark`/`packages/arksync` instructions.

## Acceptance Criteria

- AC1: Root README links ARK to `packages/ark-core/README.md`, not `packages/ark/README.md`.
- AC2: `packages/ark-core/README.md` documents current build/run/integration commands and the `@arksync/node` lifecycle.
- AC3: Docs clearly state current limitations: relay is fail-fast/not active in `ark-core-rpc`, auth is not production-grade, and direct app writes should go through ARK API/SDK.
- AC4: `TODO.md` and `ARK-P2P-WORKPROGRESS.md` are clearly marked legacy/historical and point to the current ARK README.
- AC5: Fresh verification passes: source grep for old live links, markdown file presence/content checks, and `git diff --check`.
