# Arrancador Quality 10 Hardening

## Goal

Raise `apps/arrancador` as close as practically possible to 10/10 for architecture, readability, atomicity, and testing without removing or weakening existing application capabilities.

## Baseline Snapshot

Baseline must be captured before implementation in this task directory:

- `baseline-metrics.json`
- command logs for the current verification suite when needed

## Acceptance Criteria

- AC1: No existing user-facing feature category is removed. The Electron IPC command/event surface must remain stable unless a deliberate compatibility-preserving addition is documented.
- AC2: Remaining oversized Vue route or Electron service hotspots are reduced by extracting cohesive components or pure modules with explicit typed contracts.
- AC3: Extracted logic is covered by focused unit tests, and existing behavior tests continue to pass.
- AC4: Full verification from a fresh current workspace passes:
  - `bun run typecheck`
  - `bun run lint`
  - `bun run test`
  - `bun run test:coverage`
  - `bun run build:renderer`
  - `bun run build:main`
  - `bun run build:preload`
- AC5: Final evidence includes `evidence.md`, `evidence.json`, raw command logs, and before/after metric comparison.
- AC6: If any category is still below 10/10, the remaining blocker is documented and the smallest safe follow-up is applied before claiming completion.

## Non-Goals

- No visual redesign.
- No schema or persistence format migration unless required by tests.
- No destructive git cleanup or rollback.

## Initial Component Map for Vue Work

- `LibraryPage.vue`: route-level composition surface; owns app wiring, bridge calls, navigation, and high-level orchestration.
- Proposed child component boundaries, if selected after inspection:
  - library header/status controls: present summary and primary commands via props/events.
  - library filters/search controls: present current filter/search state via explicit `v-model`/events.
  - library result grid/list: render visible games and emit selection/actions.

## Risk Controls

- Keep public prop, IPC, and bridge contracts explicit and typed.
- Prefer pure helper extraction where possible.
- Run targeted tests after each meaningful extraction and full verification at the end.
