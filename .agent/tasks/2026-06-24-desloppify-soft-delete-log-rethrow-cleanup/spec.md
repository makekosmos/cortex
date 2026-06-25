# 2026-06-24 Desloppify Soft Delete Log Rethrow Cleanup

## Classification

FULL_LOOP.

## Goal

Remove a real `LOG_AND_RETHROW` finding from Eden task soft-delete while
preserving ARK operation semantics and rejection propagation.

## Context

`softDeleteTask` caught errors only to log and rethrow the same value. Its main
caller in block-selection already logs soft-delete failures at the call site.
Other `LOG_AND_RETHROW` findings in cached task type registration paths reset
cached promises and are not no-op catches.

## Scope

In scope:

- `products/eden/src/lib/kepler-api-shim.ts`
- This task's evidence files

Out of scope:

- ARK write behavior changes
- Task object type registration retry logic
- Block-selection UI behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Soft-delete behavior is preserved.**
`softDeleteTask` still reads the task, skips missing/deleted tasks, and writes
`updatedAt`/`deletedAt` through `upsert_object`.

**AC3. Relevant checks pass.**
Typecheck, Eden build, focused kepler-api-shim browser tests, and the full scan
run are recorded.

**AC4. False positives are documented.**
Cached-promise registration catches remain because they reset retry state.
