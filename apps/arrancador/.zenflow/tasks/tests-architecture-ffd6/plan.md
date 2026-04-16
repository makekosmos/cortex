# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: 6da2204b-9f8e-4144-b85e-3a313d507627 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: 6c536d75-b374-4e4e-9d0a-38d39db5ed1d -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Delivery phases (incremental, testable milestones)
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: ba0fe160-0493-444c-b4b1-e318774804c2 -->

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

1. Break down the work into concrete tasks
2. Each task should reference relevant contracts and include verification steps
3. Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint, write tests for a module). Avoid steps that are too granular (single function) or too broad (entire feature).

If the feature is trivial and doesn't warrant full specification, update this workflow to remove unnecessary steps and explain the reasoning to the user.

Save to `{@artifacts_path}/plan.md`.

Planning output note:
- `{@artifacts_path}/spec.md` is not present in this workspace yet; the plan below is derived from `{@artifacts_path}/requirements.md` and current repo structure.
- If the Technical Specification step changes API/contracts, update the relevant plan steps before implementation.

Replace the placeholder Implementation step with actionable, test-first milestones.

### [x] Step: Baseline And Guardrails
<!-- chat-id: 4263868f-cf25-4121-bf9c-154b67907920 -->

Goal: establish a repeatable, fast feedback loop before touching logic.

- Add frontend test runner + scripts (Vitest + Testing Library + jsdom) and a minimal `pnpm test` command.
- Add Rust quality gates: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- Add a single “smoke” test per side to validate harness wiring.

Verification:
- `pnpm test`
- `pnpm run build`
- `cd src-tauri; cargo test`
- `cd src-tauri; cargo fmt --check`
- `cd src-tauri; cargo clippy -- -D warnings`

### [x] Step: Define Test Strategy And Contracts
<!-- chat-id: 88d1a7b3-616b-4077-912a-497e2b8965f5 -->

Goal: make critical behaviors explicit and hard to regress.

- Document test layers and “what goes where” (unit vs component vs integration vs E2E).
- Define contract boundaries:
  - Frontend ↔ backend: `src/lib/api.ts` invoke wrappers.
  - Backend public API: Tauri commands exposed from `src-tauri/src/lib.rs`.
- Add a lightweight “contract test table” mapping each Tauri command to inputs/outputs/error cases.

Verification:
- Plan review only (no code changes expected beyond docs/tests scaffolding).

### [x] Step: Frontend Unit And Component Tests
<!-- chat-id: 4ae596ae-acae-455e-b7b9-0b59bf100acd -->

Goal: lock down UI behavior while enabling refactors.

Scope (tests):
- Pure utilities: `src/lib/utils.ts`.
- API wrappers: `src/lib/api.ts` with mocked `invoke`.
- State layer: `src/store/GamesContext.tsx` (loading, refresh, optimistic updates if any).
- Key pages smoke/component tests (render + basic interactions):
  - `src/pages/Library.tsx`
  - `src/pages/GameDetail.tsx`
  - `src/pages/Scan.tsx`
  - `src/pages/Settings.tsx`

Notes:
- Prefer testing behaviors (visible UI state + invoked API calls), not implementation details.
- Add deterministic test fixtures for “game” objects in `src/types/index.ts`.

Verification:
- `pnpm test`

### [x] Step: Backend Unit And Integration Tests
<!-- chat-id: a947d1d4-a139-4cff-b10d-0aeee6f2623a -->

Goal: protect persistence, backup engine, and tracker from regressions.

Scope (tests):
- Database migrations/columns:
  - `src-tauri/src/database.rs` (schema invariants; migration idempotency using a temp DB).
- Game CRUD and queries:
  - `src-tauri/src/games.rs` (create/update/list; edge cases for missing paths).
- Backup engine correctness:
  - `src-tauri/src/backup/engine.rs`
  - `src-tauri/src/backup/sqoba_manifest.rs`
  - `src-tauri/src/backup/save_locator.rs`
  - Cover round-trip (manifest write/read), copy/restore integrity, error reporting.
- Tracker accounting:
  - `src-tauri/src/tracker.rs` (time accumulation rules; “process ended” handling with mocked time).

Test infrastructure approach:
- Use `tempfile` for FS isolation.
- Use a temporary SQLite file DB per test (or `:memory:` when compatible).
- Prefer “pure” functions extraction where currently everything is coupled to Tauri state.

Verification:
- `cd src-tauri; cargo test`

### [x] Step: Refactor Backend Into Testable Layers (No Behavior Change)
<!-- chat-id: f6b693cd-ce40-4c44-8212-37a9db4f54f3 -->

Goal: reduce “god modules”, make performance improvements safe.

Deliverables:
- Introduce explicit internal services/modules (examples):
  - `db` (connection + queries)
  - `domain` (types + invariants)
  - `services` (games/backup/tracker orchestration)
- Keep Tauri commands thin wrappers delegating to services.
- Add dependency injection points for:
  - DB handle
  - Clock/time provider (for tracker)
  - File system abstraction (only where it materially improves tests)

Verification:
- `cd src-tauri; cargo test`
- `cd src-tauri; cargo clippy -- -D warnings`

### [x] Step: Refactor Frontend Data Flow (No Behavior Change)
<!-- chat-id: 24c3021a-75c6-4b92-b703-43041d154c42 -->

Goal: fewer re-renders, clearer state boundaries, easier testing.

Deliverables:
- Make `src/store/GamesContext.tsx` the single source of truth for library state (or split into focused contexts if it’s currently overloaded).
- Memoize derived lists/selectors and avoid repeated expensive computations in render.
- Move side-effectful logic out of pages into hooks (e.g., `useGames`, `useGameDetail`).
- Ensure router/layout composition remains identical.

Verification:
- `pnpm test`
- `pnpm run build`

### [x] Step: Performance Pass With Benchmarks
<!-- chat-id: d87a663f-acc4-4e11-a954-3d02cb61485b -->

Goal: measurably faster while staying identical functionally.

Approach:
- Establish baseline timings for:
  - Library load time (initial list)
  - Scan duration
  - Backup create/restore duration on representative dataset
- Optimize only with evidence; keep changes behind tests.

Likely optimizations (confirm via profiling):
- Reduce redundant backend calls by caching/ batching in `src/lib/api.ts` call sites.
- Avoid unnecessary serialization/deserialization on hot paths.
- Reduce filesystem walk overhead in scan/backup (e.g., smarter filtering, parallelism with Rayon where safe).

Verification:
- Run the same benchmark scripts before/after and record results in the task artifacts.

### [x] Step: End-To-End Smoke Tests (Optional But Recommended)
<!-- chat-id: ec0016f8-5579-48bb-9341-9dc19557049b -->

Goal: catch “wiring” regressions that unit tests can’t.

- Add minimal Playwright (or equivalent) smoke flows:
  - App starts and renders Library
  - Navigate to Settings and back
  - Trigger Scan flow with a stubbed backend (if full Tauri E2E is too heavy)

Verification:
- `pnpm test:e2e` (or documented manual run steps)

### [x] Step: CI And Regression Gates
<!-- chat-id: 356e8032-a8fd-42a7-b601-304dc4757684 -->

Goal: prevent future bugs from landing.

- Add CI workflow (GitHub Actions or current CI) to run:
  - Frontend: typecheck/build + tests
  - Backend: fmt/clippy/test
- Add coverage reporting (at minimum: Vitest coverage; optionally Rust coverage) with a non-blocking first threshold, then ratchet upwards.

Verification:
- CI green on main branches.
