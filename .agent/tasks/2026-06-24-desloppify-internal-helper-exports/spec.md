# 2026-06-24 Desloppify Internal Helper Exports

## Classification

FULL_LOOP.

## Goal

Reduce `DEAD_EXPORT` noise by removing export surface from helpers/types that
are only used within their defining file, while preserving actual runtime
behavior and public contracts that have external imports.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 496 total
- Severity: critical 0, high 295, medium 130, low 71

This slice intentionally avoids public contract files, ARK sync types, and
focus launcher code. Eden typed-note docs were checked because two scoped files
format object/image values; the changes do not alter type metadata, ARK calls,
or rendered behavior.

## Scope

In scope:

- `incubator/akasha/src/lib/libraryView.ts`
- `incubator/arrancador/src/lib/arkGames.ts`
- `products/delphi/src/services/recurrence/recurrence.ts`
- `products/eden/src/lib/objectFieldFormatting.ts`
- `products/eden/src/lib/localImages.ts`
- `products/eden/src/composables/usePreferences.ts`
- `packages/visuals/components/GamePosterCard.vue`
- `platform/desktop/src/views/settings/tabs/AppCommandsTab.vue`
- This task's evidence files

Out of scope:

- ARK/sync schema or write paths
- Eden typed-note metadata changes
- Focus launcher commands
- Public IPC/Raycast package contracts
- UI layout or styling changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped internal-only exports are removed.**
Reference checks prove the changed symbols have no external imports, and the
scoped `DEAD_EXPORT` findings disappear.

**AC3. Runtime behavior is preserved.**
Helpers still called internally remain available locally; externally imported
types/APIs remain exported.

**AC4. Relevant checks pass.**
The affected desktop/extensions builds and type checks pass, or any unrelated
blocker is documented with exact command output.
