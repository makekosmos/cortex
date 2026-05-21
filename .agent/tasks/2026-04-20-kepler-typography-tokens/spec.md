# Task Spec - Kosmos shared typography tokens

## Goal

Introduce global typography variables in `kosmos-visuals` for the primary title scale and switch Eden's first-level headings to use them instead of local hardcoded values.

## Scope

- `packages/kosmos-visuals/theme/css-variables.css`
- `apps/eden/ts/src/Editor.css`
- `apps/eden/ts/src/App.css`
- `apps/eden/ts/src/components/settings/SettingsPage.css`
- `apps/eden/ts/src/components/spaces/SpacesView.css`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeEditor.vue`

## Acceptance Criteria

- AC1: `kosmos-visuals` defines a shared global typography token set for the primary title scale.
- AC2: Eden note/object title input uses the shared primary title token.
- AC3: Eden settings page titles use the shared primary title token.
- AC4: Other Eden first-level page/editor headings that already represent the same visual tier use the same shared token instead of local hardcoded values.
- AC5: Eden typecheck and production build pass after the token migration.

## Verification Plan

- `bun x tsc --noEmit`
- `bun run build`

## Raw Artifact Targets

- `.agent/tasks/2026-04-20-kosmos-typography-tokens/raw/tsc.txt`
- `.agent/tasks/2026-04-20-kosmos-typography-tokens/raw/build.txt`
