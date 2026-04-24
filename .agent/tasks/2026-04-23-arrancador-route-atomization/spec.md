# Arrancador Route Atomization Spec

Task ID: `2026-04-23-arrancador-route-atomization`

## Goal

Continue raising Arrancador toward an optimal maintainable state by reducing route-page responsibility and moving pure business/display logic into focused, tested modules.

This pass builds on `2026-04-23-arrancador-quality-hardening` and targets the remaining high-leverage route-page issues without rewriting UI markup.

## Acceptance Criteria

AC1. Library filtering and sorting pure logic is extracted from `LibraryPage.vue`.

- A dedicated module owns parsing numeric filters, splitting metadata lists, active-filter predicates/counting, and game filtering/sorting.
- `LibraryPage.vue` delegates to that module instead of carrying the full implementation inline.

AC2. Game detail display helpers are extracted from `GameDetailPage.vue`.

- A dedicated module owns playtime/bytes formatting, description normalization, save-path template resolution, and play-status labels/tones.
- `GameDetailPage.vue` delegates to that module.

AC3. Extracted logic has focused unit tests.

- Tests cover library filter behavior, sort modes, active-filter counting, display formatting, description cleanup, and save-path template resolution.

AC4. Existing behavior remains verified.

- Run `bun run typecheck`.
- Run `bun run test`.
- Run `bun run biome:check`.
- Attempt `bun run test:e2e`; if blocked by local Playwright spawn permissions, record the blocker.

AC5. Proof artifacts are recorded.

- Create `evidence.md`, `evidence.json`, and raw command artifacts under `.agent/tasks/2026-04-23-arrancador-route-atomization/`.

## Non-Goals

- Do not redesign page UI.
- Do not split every visual subsection in this pass.
- Do not alter unrelated dirty worktree changes.
- Do not claim final 10/10 unless verification supports it.
