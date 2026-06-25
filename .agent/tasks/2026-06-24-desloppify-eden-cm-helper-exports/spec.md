# 2026-06-24 Desloppify Eden CM Helper Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused Eden CodeMirror/block-selection helper exports while
preserving the editor extension APIs that are imported by `CmEditor` and tests.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 479 total
- Severity: critical 0, high 278, medium 130, low 71

Eden editor docs were checked. This slice does not change markdown content
storage, autosave, typed-note metadata, lazy loading, or editor UI behavior.

## Scope

In scope:

- `products/eden/src/editor-cm/cmGate.ts`
- `products/eden/src/editor-cm/cm/list-indent.ts`
- `products/eden/src/editor-cm/cm/ordered-list-renumber.ts`
- `products/eden/src/editor-cm/cm/slash-commands.ts`
- `products/eden/src/lib/blockSelectionPointer.ts`
- This task's evidence files

Out of scope:

- `CmEditor.vue` behavior changes
- Typed-note metadata or object rendering
- Markdown content codec
- ARK data/write paths
- Broad block-selection feature removal

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped CM/helper exports are removed.**
Reference checks prove the changed symbols have no external imports, and the
scoped `DEAD_EXPORT` findings disappear.

**AC3. Editor APIs remain available.**
APIs imported by `CmEditor` and focused tests remain exported and pass focused
checks.

**AC4. Relevant checks pass.**
Eden build and focused CM/block-selection tests pass, or any unrelated blocker
is documented with exact command output.
