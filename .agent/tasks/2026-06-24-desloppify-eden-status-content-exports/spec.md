# 2026-06-24 Desloppify Eden Status Content Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused Eden-local exports from task status, markdown content,
and generated-title helpers without changing editor content encoding, task
normalization, or title behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 488 total
- Severity: critical 0, high 287, medium 130, low 71

Large remaining Eden groups such as `typedNotes.ts`, CM editor internals, and
markdown import parsing are intentionally left for separate review. This slice
only touches symbols that are either used locally or have no callers.

## Scope

In scope:

- `products/eden/src/lib/taskStatus.ts`
- `products/eden/src/editor-cm/content.ts`
- `products/eden/src/lib/entryTitles.ts`
- This task's evidence files

Out of scope:

- Eden typed-note metadata
- Markdown frontmatter import parser behavior
- CM editor plugins
- ARK data/write paths
- UI layout or styling

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped Eden-local exports are removed.**
Reference checks prove the changed symbols have no external imports, and the
scoped `DEAD_EXPORT` findings disappear.

**AC3. Runtime behavior is preserved.**
Task normalization, markdown content read/write, and generated-title display
helpers continue to build and use the same local constants/helpers.

**AC4. Relevant checks pass.**
Eden build and focused content/status tests pass, or any unrelated blocker is
documented with exact command output.
