# Arrancador Full 10/10 Architecture Spec

Task ID: `2026-04-24-arrancador-full-10`

## Goal

Make Arrancador reach a defensible 10/10 scorecard across architecture, readability, atomicity, and testing using measurable before/after evidence.

This task continues from `2026-04-24-arrancador-architecture-10`. The user explicitly asked not to return until the score table can show 10/10 in every category.

## Rubric

The final score table can show 10/10 only if all criteria below are PASS.

### Architecture 10/10

- Route-level Vue pages that orchestrate critical flows delegate side effects to composables.
- Electron IPC handlers register channels and delegate to application services; they do not own DB/filesystem/domain workflows.
- Core application workflows expose explicit dependency ports for tests.
- Architecture boundary tests guard the above constraints.

### Readability 10/10

- New or recently touched architecture-critical modules are split by responsibility.
- `backup-workflow.ts` must be a small public façade, not a 400+ line workflow implementation.
- The backup workflow implementation must separate:
  - dependency wiring,
  - public contracts,
  - query/read use cases,
  - mutation/write use cases,
  - response/progress mapping.
- `GameDetailPage.vue` remains below the existing route budget and has no launch/backup preflight implementation logic.

### Atomicity 10/10

- DB write groups in backup workflows remain centralized in application workflow functions.
- Settings updates stay transactional.
- Backup create workflow has one clear ordered write path: artifact creation, DB insert, game state update, reconciliation, one-shot setting reset, achievement event.
- Tests prove the ordered write path and missing backup rejection.

### Testing 10/10

- Fresh verification commands all pass:
  - `bun run typecheck`
  - `bun run test`
  - `bun run test:coverage`
  - `bun run test:e2e`
  - `cargo test --manifest-path sidecar\Cargo.toml`
- Test count must not decrease from the previous pass baseline of 119.
- Coverage must remain above configured thresholds after all extracted modules are included.
- Architecture guardrail tests must cover backup IPC and Game Detail route orchestration.

## Baseline Before This Pass

Captured after the previous architecture pass:

- Tests: 37 files / 119 tests.
- Coverage: statements 79.5%, branches 73.09%, functions 74.52%, lines 80.88%.
- `backup-workflow.ts`: 499 lines.
- `backup-handlers.ts`: 151 lines and thin by responsibility.
- `GameDetailPage.vue`: 392 lines with launch workflow delegated to `useGameLaunchFlow`.
- `games.ts`: 395 lines.

## Acceptance Criteria

AC1: Freeze this spec before implementation.

AC2: Refactor backup workflow into responsibility-focused modules.
- `apps/arrancador/electron/main/services/backup-workflow.ts` is a façade under 80 lines.
- Backup workflow contracts/types live outside the façade.
- Read/query backup use cases and write/mutation backup use cases live in separate files.
- Existing imports from `../services/backup-workflow` keep working.

AC3: Preserve behavior.
- Existing IPC channels and renderer API behavior remain compatible.
- Existing backup workflow tests still pass.
- Existing Game Detail launch flow tests still pass.

AC4: Strengthen measurable architecture guardrails.
- Add/adjust architecture tests so `backup-workflow.ts` cannot regress into a large implementation file.
- Add/adjust architecture tests so Game Detail route stays under its composition-surface budget.

AC5: Produce before/after metrics.
- Store baseline metrics in `baseline-metrics.json`.
- Store final metrics in `final-metrics.json`.
- `evidence.md` must include a before/after score table with 10/10 for every category and the measurable basis for each score.

AC6: Fresh verification PASS.
- All five verification commands pass against current code.

## Stop Condition

Do not claim completion unless AC1-AC6 are PASS and the final evidence table can honestly show 10/10 in all four requested categories under the rubric above.
