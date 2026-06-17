# Task Spec — 2026-06-17 kepler sync settings prod

## Task ID / Path

- `2026-06-17-kepler-sync-settings-prod`
- `.agent/tasks/2026-06-17-kepler-sync-settings-prod/spec.md`

## Original task statement

> Создай frozen proof-loop spec для задачи production-ready UI синхронизации в Kepler Settings на основе полного ТЗ пользователя из предыдущего сообщения. Нужно сохранить spec в .agent/tasks/<DATE>-kepler-sync-settings-prod/spec.md (или ближайший project convention). Включи acceptance criteria, constraints/forbidden rules, implementation slices, verification plan, rollback/safety notes. Не реализуй код. Используй релевантные docs по FULL_LOOP/proof-loop/write-boundary/sync/testing. Верни путь spec и краткое резюме.

## Context

Kepler Settings already exists as a dedicated window and preload bridge, but the requested work is a production-ready UI synchronization flow for Settings that must be safe for prod builds and compatible with the existing Kepler/Electron/ARK boundaries. The feature touches UI, Electron main/preload API surface, iroh-backed runtime behavior, and sync state semantics, so this is a `FULL_LOOP` task and must stay within a frozen proof-loop spec before implementation.

## Scope

### In scope

- Production iroh build/runtime behavior needed for the Settings sync flow.
- ARK RPC-backed sync snapshots and lifecycle actions:
  - `snapshot`
  - `disconnect`
  - `connect_with_code`
- Electron main/preload API additions or wiring needed to expose the sync UI safely.
- Visuals-layer component(s) in `@kosmos/visuals` used by Settings UI.
- Kepler Settings UI and/or modal flow for sync management.
- Documentation updates required to explain the sync UI contract and any operator/user-visible behavior.
- Tests covering the Settings sync flow, API contract, and safety regressions.

### Out of scope

- Reworking unrelated Kepler settings tabs.
- Changing ARK data model, schema, or sync protocol beyond what is required for the Settings sync UI contract.
- Broad product-wide UI redesign.
- Any direct SQL writes to ARK tables from app services or renderer code.
- Non-production debug-only shortcuts that would weaken the sync contract in shipped builds.

## Assumptions

- “Production-ready” means the feature must be safe in shipped Kepler builds, not just usable in dev/test mode.
- The sync UI is expected to reflect real backend state, not a mocked-only flow.
- `connect_with_code` is the canonical user-facing join path unless the underlying product contract explicitly says otherwise.
- If a modal is required, it must be part of the Settings UX and not a separate hidden workflow.

## Constraints and forbidden rules

- Do not change production code in this task; only freeze the spec.
- Do not bypass the ARK write boundary; no raw SQL writes from renderer or Electron main.
- Do not assume sync is always on; follow the existing opt-in sync semantics.
- Do not weaken headless/test-mode behavior for any window or modal.
- Do not introduce new destructive migrations or breaking sync-state changes.
- Do not add speculative APIs that are not required by the stated settings sync contract.
- Keep the task isolated to the smallest defensible feature slice.

## Implementation slices

### Slice 1 — Contract and backend surface

Define the exact sync state contract that the Settings UI needs from production iroh/ARK runtime, including snapshot fields, disconnected state, and connect/join behavior.

### Slice 2 — Electron API wiring

Expose only the minimal preload/main IPC needed for the Settings flow, with clear ownership and no direct renderer access to backend internals.

### Slice 3 — Visuals component

Add or extend shared `@kosmos/visuals` primitives needed to render sync state, connection actions, and modal content consistently.

### Slice 4 — Kepler Settings UI / modal

Implement the user-facing Settings surface for viewing sync state, disconnecting, and joining with code, with prod-safe copy and disabled/error states.

### Slice 5 — Docs and tests

Document the behavior and verify the contract with unit/integration/e2e coverage, including headless safety and regressions around connection lifecycle.

## Acceptance Criteria

**AC1.** The spec explicitly covers the production iroh build/runtime path for Kepler Settings sync and defines how the UI reads the authoritative sync state.

**AC2.** The spec explicitly covers ARK RPC snapshot, `disconnect`, and `connect_with_code` behavior as required by the Settings sync UI contract.

**AC3.** The spec explicitly covers the Electron API surface needed to reach the sync backend from the Settings UI without violating renderer isolation.

**AC4.** The spec explicitly covers the shared `@kosmos/visuals` component(s) required for the Settings sync UI or modal.

**AC5.** The spec explicitly covers the Kepler Settings UI/modal flow, including loading, connected, disconnected, error, and retry states.

**AC6.** The spec includes constraints/non-goals that forbid direct SQL writes, boundary violations, and unsafe prod-only behavior.

**AC7.** The spec includes a verification plan with concrete commands and a Definition of Done.

**AC8.** The spec includes rollback/safety notes for failed sync state, disconnect failure, and modal/API regressions.

## Definition of Done

- The implementation can be verified against the frozen ACs without ambiguity.
- The final diff remains within the intended settings sync slice.
- The feature is safe for production builds and does not weaken ARK/sync boundaries.
- Verification includes automated checks plus at least one user-facing/manual validation path for the Settings flow.

## Verification plan

- Read the frozen spec against the requested behavior and confirm each AC is testable.
- Verify relevant repo checks for the touched surfaces:
  - typecheck / build for the desktop app(s)
  - targeted unit/integration tests for Electron/API/visuals contracts
  - targeted UI/e2e checks for the Settings flow
  - ARK write-boundary guard for any touched app-service paths
- Validate prod/headless safety for any window or modal behavior.

## Rollback / safety notes

- If any sync action cannot be proven safe in prod, the feature must fail closed and keep the existing Settings UI intact.
- Connection and disconnect actions must remain reversible at the UX level where possible.
- Any API surface added for this task should be minimal and removable without cross-app fallout.
- If implementation discovers broader protocol or product decisions, stop and split into a separate task instead of widening this one.

## Proof-loop notes

- This spec is frozen before implementation and should not be edited during the loop except for an explicit task split.
- Evidence must be collected per AC after implementation.
- If verification fails, the next artifact is `problems.md`, not a spec rewrite.

## Verification commands (planned)

- `bun run docs:check` — sanity-check documentation references if docs are updated.
- `bun run ark:guard:writes` — ensures no forbidden ARK SQL writes were introduced.
- `bun run --cwd platform/desktop typecheck` — validates Electron/Settings/API wiring.
- `bun run --cwd platform/desktop test` or targeted e2e/spec command for the Settings sync flow — validates UI/runtime behavior.
- Any additional repo-specific verifier required by the chosen implementation slice.
