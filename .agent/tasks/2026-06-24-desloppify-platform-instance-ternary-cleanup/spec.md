# 2026-06-24 Desloppify Platform Instance Ternary Cleanup

## Classification

FULL_LOOP.

## Goal

Remove local `NESTED_TERNARY` findings from Electron preload and instance slot
resolution without changing derived values.

## Context

The findings were pure derivation logic:

- platform marker: `darwin -> mac`, `win32 -> windows`, else `linux`
- instance kind: `test`, `prod`, or `dev`
- product name by slot/kind
- hotkey by slot/platform

## Scope

In scope:

- `platform/desktop/electron/extension-preload.ts`
- `platform/desktop/electron/preload.ts`
- `platform/desktop/electron/instance.ts`
- This task's evidence files

Out of scope:

- Preload splitting
- Instance env centralization
- Electron startup behavior changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Derived values are preserved.**
Platform markers, instance kind, product names, and hotkeys are equivalent to
the previous ternary expressions.

**AC3. Relevant checks pass.**
Typecheck, shell build, and the full scan run are recorded.
