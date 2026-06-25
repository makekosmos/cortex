# 2026-06-24 Desloppify Eden Local Type Export Cleanup

## Classification

FULL_LOOP.

## Goal

Remove unused Eden local type exports that have no runtime behavior and no
callers.

## Context

After the Markdown frontmatter cleanup, the scan still reported local
`DEAD_EXPORT` findings in Eden sidebar/block-selection files. Reference checks
showed some are false positives or require separate behavioral proof:

- `sortEntries` is imported by `SpacesView.vue`.
- `DragRect` is imported by `BlockSelectionOverlay.vue`.
- `useBlockSelection` itself needs a separate editor/UI behavior audit.

The unused `DragPayload` and `UseBlockSelectionReturn` type exports have no
callers and no runtime semantics.

## Scope

In scope:

- `products/eden/src/components/sidebar/types.ts`
- `products/eden/src/composables/useBlockSelection.ts`
- This task's evidence files

Out of scope:

- Sidebar sort behavior
- Block-selection runtime behavior
- Editor UI/e2e behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Only dead local type exports are removed.**
`DragPayload` and `UseBlockSelectionReturn` are removed; runtime exports stay
unchanged.

**AC3. Relevant checks pass.**
Typecheck, Eden build, focused block-selection browser spec, and the full scan
run are recorded.

**AC4. False positives are documented.**
Remaining scoped findings with real imports or higher behavioral risk are
called out explicitly.
