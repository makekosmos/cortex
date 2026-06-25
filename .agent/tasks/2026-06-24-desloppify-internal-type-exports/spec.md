# 2026-06-24 Desloppify Internal Type Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused type-only exports from internal UI/facade modules while
preserving runtime behavior and externally imported public APIs.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 472 total
- Severity: critical 0, high 271, medium 130, low 71

This slice intentionally avoids package/API boundaries with higher false
positive risk such as `platform/desktop/shared/ipc-types.ts`,
`@raycast/api`, `core/ark`, and Delphi sync types.

## Scope

In scope:

- `packages/visuals/components/SyncNodeRow.vue`
- `platform/desktop/src/views/settings/composables/useDictationConfig.ts`
- `products/eden/src/editor-cm/vimMotions.ts`
- `incubator/arrancador/src/lib/arrancadorApi.ts`
- This task's evidence files

Out of scope:

- IPC/shared package contracts
- ARK client or sync types
- Raycast API package types
- Runtime behavior changes
- UI layout or visual changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped type-only exports are removed.**
Reference checks prove the changed type symbols have no external imports, and
the scoped `DEAD_EXPORT` findings disappear.

**AC3. Public APIs remain available.**
Runtime functions and externally imported types/constants continue to be
exported.

**AC4. Relevant checks pass.**
Desktop typecheck and affected extension builds pass, or any unrelated blocker
is documented with exact command output.
