# 2026-06-24 Desloppify Typed Notes Error Cause

## Classification

FULL_LOOP.

## Goal

Remove the `CATCH_WRAP_NO_CAUSE` finding from Eden typed-note JSON parsing while
preserving the existing error message.

## Context

`parseJsonWithContext` wraps JSON parse failures with domain context such as
`note type schema`. The wrapped error should preserve the original parse error
as `cause`.

## Scope

In scope:

- `products/eden/src/lib/typedNotes.ts`
- This task's evidence files

Out of scope:

- Typed object schema exports
- System type definitions
- Header prop/schema behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Error message is preserved.**
The thrown message remains `${label}: invalid JSON (${message})`, with the
original error attached as `cause`.

**AC3. Relevant checks pass.**
Typecheck, focused system type tests, Eden build, and the full scan run are
recorded.
