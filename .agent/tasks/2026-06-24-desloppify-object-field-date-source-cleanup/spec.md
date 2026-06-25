# 2026-06-24 Desloppify Object Field Date Source Cleanup

## Classification

FULL_LOOP.

## Goal

Remove a local `NESTED_TERNARY` finding from Eden object field date formatting
without changing formatted output.

## Context

`formatReadableRussianDate` selected the date source with a nested ternary:
number values are used directly, string values are parsed with `Date.parse`,
and unsupported values become `NaN`.

## Scope

In scope:

- `products/eden/src/lib/objectFieldFormatting.ts`
- This task's evidence files

Out of scope:

- Date formatting copy
- Object field rendering behavior
- Typed note schema changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Behavior is preserved.**
Number, string, and unsupported values map to the same date source semantics as
before.

**AC3. Relevant checks pass.**
Typecheck, Eden build, and the full scan run are recorded.
