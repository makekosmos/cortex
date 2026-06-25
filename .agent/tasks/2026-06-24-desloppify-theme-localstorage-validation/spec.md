# 2026-06-24 Desloppify Theme LocalStorage Validation

## Classification

FULL_LOOP.

## Goal

Remove `LOCALSTORAGE_CAST` findings from Eden and Delphi theme modules by
validating stored theme values before hydration.

## Context

Both theme modules read `vite-ui-theme` at module load and cast it to
`"light" | "dark" | "system"`. Unknown user-controlled values should fall back
to the same defaults as before:

- Eden: `dark`
- Delphi: `system`

## Scope

In scope:

- `products/eden/src/composables/useTheme.ts`
- `products/delphi/src/composables/useTheme.ts`
- This task's evidence files

Out of scope:

- Theme UI behavior changes
- Storage key migration
- Visual redesign

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Storage validation is explicit.**
Only `light`, `dark`, and `system` are accepted from `localStorage`; invalid or
missing values use the previous fallback defaults.

**AC3. Relevant checks pass.**
Typecheck, Eden build, Delphi build, and the full scan run are recorded.
