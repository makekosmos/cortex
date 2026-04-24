# Evidence

## Scope

This task fixed Eden search so it no longer ignores Ark-backed notes/objects and moved the Eden search surface onto the shared Kepler Visuals command palette contract.

## Implemented Changes

- Added Ark backend search RPC in Rust:
  - `packages/ark-core/rust/src/main.rs`
  - `packages/ark-core/rust/src/db.rs`
- Merged Ark + heart search results in Eden main process:
  - `apps/eden/ts/main/store.ts`
- Replaced the local standalone Eden search overlay shell with shared `@kepler/visuals` `CommandPalette`:
  - `apps/eden/ts/src/components/SearchOverlay.vue`
  - `packages/kepler-visuals/components/CommandPalette.vue`
- Updated Eden search e2e locators and made the Ark-backed expectation explicit:
  - `apps/eden/ts/tests/app.spec.ts`

## Functional Findings

- Root cause for "search finds nothing":
  - Eden currently saves normal notes as Ark `note_obj` objects by default.
  - Eden search previously queried only `eden-heart`.
  - Result: visible current notes existed in the UI but were absent from search.
- Root cause for the off-looking search UI:
  - Eden shell already used shared Kepler Visuals primitives.
  - The search surface itself still used a local `SearchOverlay.vue` + custom CSS instead of the shared visuals command palette.

## Verification

### PASS

- `bun run lint`
  - cwd: `apps/eden/ts`
- `bun run build`
  - cwd: `apps/eden/ts`
  - note: build completes successfully on current code; Rust emits an existing `dead_code` warning for unused relay fields in `ark-core-rpc`
- Direct backend integration proof for both search backends
  - artifact: `artifacts/backend-search-verification.json`
  - proved:
    - `eden-heart` search finds a legacy note
    - Ark `search_objects` finds an Ark-backed `note_obj`
    - merged result set contains both entries
- Source-level UI integration proof
  - artifact: `artifacts/source-integration-check.txt`
  - proved:
    - Eden search UI imports `CommandPalette` from `@kepler/visuals`
    - shared `CommandPalette` exposes controlled query + test ids needed by Eden
    - Eden main search path now calls Ark `search_objects`

### Environment Gap

- Browser e2e attempt for search
  - artifact: `artifacts/playwright-search-attempt.txt`
  - status: blocked by local environment
  - failure: Playwright worker startup hits `spawn EPERM` before running the app, so this is not evidence of an application regression

## Acceptance Criteria Status

- AC1: PASS
- AC2: PASS
- AC3: PASS
- AC4: PASS

## Residual Risk

- The updated browser test path was not executed end-to-end because Playwright cannot fork workers in this environment.
- Runtime UX risk is therefore reduced by build verification and direct backend proof, but not fully eliminated by browser automation in this session.

## Encoding Check

- Proof-loop documents in this task folder are ASCII.
