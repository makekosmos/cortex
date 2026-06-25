# 2026-06-24 Desloppify Script Theme Store Exports

## Classification

FULL_LOOP.

## Goal

Remove a small set of confirmed unused exports from release tooling, Eden theme
side-effect setup, Dashboard store state, and Delphi Pinia store while
preserving active runtime behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 456 total
- Severity: critical 0, high 255, medium 130, low 71

Distribution forbidden docs were checked before touching `release-version.mjs`.
This slice does not change versions, release channels, publishing behavior, or
wire formats.

## Scope

In scope:

- `platform/desktop/scripts/release-version.mjs`
- `products/eden/src/composables/useTheme.ts`
- `platform/desktop/src/dashboard/store.ts`
- `products/delphi/src/store/todos.ts`
- This task's evidence files

Out of scope:

- Version bumps or release publishing
- Dashboard UI layout changes
- Delphi store behavior changes
- Theme behavior changes
- Public package contracts

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped unused exports are removed.**
Reference checks prove scoped symbols have no external imports, and scoped
`DEAD_EXPORT` findings disappear.

**AC3. Runtime behavior is preserved.**
Release-version CLI read path, Eden theme side effect, Dashboard store behavior,
and named Delphi store imports continue to build/check.

**AC4. Relevant checks pass.**
Typecheck, relevant extension builds, and focused regression checks pass, or any
unrelated blocker is documented with exact command output.
