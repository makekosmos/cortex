# Technical Specification: Tests + Architecture (Arrancador)

## 0) Scope of This Spec

This document describes how to:
- Add comprehensive automated testing (frontend + Rust backend)
- Improve architecture for maintainability and performance
- Keep **identical user-facing functionality**

Implementation details are intentionally staged and incremental to reduce regression risk.

## 1) Technical Context

### Frontend
- React 18 + TypeScript + Vite
- Router: `react-router-dom`
- UI: Tailwind + Radix UI
- Tauri bridge: `src/lib/api.ts` using `@tauri-apps/api/core.invoke`
- State: Context (`src/store/GamesContext.tsx`)

### Backend
- Tauri 2 (Rust)
- SQLite via `rusqlite` (bundled)
- Background work: `tokio`, `rayon`, filesystem scanning (`jwalk`, `walkdir`)
- External integrations: RAWG (currently `reqwest` blocking)
- Key modules:
  - DB schema/migrations: `src-tauri/src/database.rs`
  - Game CRUD + launch/process mgmt: `src-tauri/src/games.rs`
  - Tracker: `src-tauri/src/tracker.rs`
  - Backup orchestration/engine: `src-tauri/src/backup.rs`, `src-tauri/src/backup/*`
  - System & scan: `src-tauri/src/system.rs`, `src-tauri/src/scan.rs`

### Current Gaps
- No configured test runner on the frontend (no Vitest/Jest)
- Rust has `tempfile` in dev-deps but limited/unknown test coverage
- Some high-risk areas are IO-heavy and concurrency-heavy (scan/backup/tracker/process mgmt)

## 2) Goals and Non-goals

### Goals
1. Better performance (fewer UI re-renders, fewer redundant invokes, safer background work)
2. Identical functionality and UX
3. Strong regression protection via automated tests
4. Architectural clarity: clear boundaries between UI, domain logic, and IO
5. Safer evolution: typed contracts, consistent error handling, deterministic tests

### Non-goals
- Feature redesign or UI redesign
- Switching database or platform
- Large rewrites without incremental checkpoints

## 3) Architecture Improvements (No Behavior Change)

### 3.1 Frontend architecture

#### Current
- Pages/components call `gamesApi/backupApi/...` directly
- Shared state and side-effects likely concentrated in `GamesContext`

#### Target structure (incremental)

Add a small set of explicit layers while keeping React patterns familiar:

1) **API layer** (existing)
- Keep `src/lib/api.ts` as the only Tauri invoke boundary.
- Add a thin error/timeout normalization wrapper (still returning same shapes).

2) **Data layer** (new)
- Introduce `src/services/*` or `src/data/*` for higher-level operations and caching.
  - Examples: `src/services/gamesService.ts`, `src/services/backupService.ts`
- Responsibilities:
  - Coalesce repeated calls (memoized `getAll`, cache by `gameId`)
  - Deduplicate inflight requests
  - Convert date strings/number formatting in one place (if applicable)

3) **Domain/selectors layer** (new)
- Pure functions that compute derived data for UI:
  - Filtering/sorting library list
  - Mapping playtime statistics buckets
  - Formatting and mapping backup status
- Lives in `src/domain/*` and is 100% unit-testable.

4) **UI state**
- Keep Context, but tighten it:
  - Stable context value via `useMemo`
  - Avoid storing derived data in state
  - Add selectors/hooks: `useGames()`, `useGame(id)`, etc.

#### Performance-focused UI changes (no UX changes)
- Ensure `GamesContext` doesn’t re-render the whole tree on small updates
- Prefer memoized selectors and stable callbacks
- For large libraries: consider list virtualization (e.g. `react-window`) behind a flag if needed

### 3.2 Backend architecture

#### Current
- Tauri commands and core logic are likely mixed inside module files

#### Target structure (incremental)

1) **Command layer** (existing entrypoints)
- Keep Tauri command names stable (to preserve frontend behavior).
- Commands become thin: validate inputs, call service, map errors.

2) **Service layer** (new)
- Create focused services:
  - `GameService` (CRUD, launch, process bookkeeping)
  - `BackupService` (orchestration + engine calls)
  - `MetadataService` (RAWG)
  - `StatsService` (aggregations)
  - `SettingsService`

3) **Repository/DAO layer** (new)
- Isolate SQL + migrations + queries:
  - `GameRepo`, `BackupRepo`, `SettingsRepo`
- Prefer prepared statements and explicit transactions for batch ops.

4) **Common error model** (new)
- Add `thiserror`-based error enums per subsystem.
- Provide a single conversion to the Tauri error boundary (string/serializable error).
- Normalize:
  - Not found
  - Validation
  - IO error
  - DB constraint
  - External API

#### Performance and reliability changes (no behavior change)
- Ensure heavy IO/CPU work runs off the main Tauri thread:
  - Use `tauri::async_runtime::spawn_blocking` for blocking IO (filesystem, rusqlite)
  - Use `rayon` only where it measurably improves scan/backup
- RAWG integration:
  - Prefer async `reqwest` usage to avoid blocking (or wrap blocking calls in `spawn_blocking`)
  - Add request timeouts and deterministic error mapping
- SQLite improvements:
  - Add indexes for frequently queried columns (e.g. by name, favorite, last_played, exe_path)
  - Keep migrations in `src-tauri/src/database.rs` (ensuring existing DB upgrades)

## 4) Testing Strategy

### 4.1 Frontend tests

#### Tooling
- Add `vitest` + `@testing-library/react` + `@testing-library/user-event`
- Test environment: `jsdom`
- Mock Tauri invoke boundary:
  - Unit/component tests should mock `@tauri-apps/api/core.invoke`
  - Prefer a single test helper in `src/test/tauriMock.ts`

#### What to test
1) **Domain pure functions** (`src/domain/*`)
- Sorting/filtering/search logic
- Date/playtime formatting and bucketing
- Backup status computations

2) **Components/pages**
- Smoke + interaction tests for:
  - `src/pages/Library.tsx` (search/filter/favorite)
  - `src/pages/GameDetail.tsx` (launch, backup/restore buttons, metadata apply)
  - `src/pages/Settings.tsx` (settings read/write)
  - `src/pages/Statistics.tsx` (renders stats given mocked data)
  - `src/pages/SystemInfo.tsx` (disk test flow given mocked responses)

3) **Error states**
- Failed invoke results, empty states, loading states

#### Contract tests for `src/lib/api.ts`
- Add a small set of tests that verify:
  - Correct invoke command name per method
  - Correct argument mapping
This catches silent regressions when renaming Rust commands.

### 4.2 Backend tests (Rust)

#### Test types
1) **Unit tests** for pure logic
- Backup manifest building/parsing
- Path normalization and restore diff logic
- Playtime aggregation logic

2) **Integration tests** against SQLite
- Use `tempfile` DB path to test:
  - Migrations
  - CRUD
  - Search
  - Constraints

3) **Behavioral tests** for services
- Mock file operations where feasible
- For filesystem-heavy behavior, use a temp directory fixture

#### Testability enablers
- Extract filesystem/process execution behind small traits:
  - `ProcessRunner` (launch/kill)
  - `Fs` helpers (copy/list)
Provide real + test implementations.

### 4.3 E2E (optional, gated)

If needed for critical flows (backup/restore/launch), add Playwright later.
- Run only in CI environments that support it.
- Keep E2E count small; rely on unit/integration for breadth.

## 5) API / Interface Changes

### Frontend ↔ Backend
- Keep existing Tauri command names stable.
- Standardize error payloads internally; if the frontend currently expects strings, keep strings.

### Internal module interfaces
- Add service/repo boundaries.
- Prefer explicit input/output structs (serde-serializable) at command boundary.

## 6) Data Model / Migrations

- No user-visible schema changes required for this initiative.
- Add/adjust indexes and missing columns via migrations in `src-tauri/src/database.rs`.
- Add a migration test that:
  - Creates an “old” schema fixture
  - Runs `ensure_*` migration
  - Asserts new columns/indexes exist

## 7) Delivery Phases (Incremental, Testable)

Phase 1 — Baseline and safety net
- Add frontend test runner + minimal smoke tests
- Add Rust test harness for DB and backup engine
- Add CI-like scripts (local) to run tests/typecheck

Phase 2 — Architectural extraction (no behavior change)
- Frontend: introduce domain + services layers; keep UI stable
- Backend: introduce services + repos; commands call services
- Add/expand tests as code moves into pure/testable modules

Phase 3 — Performance work with measurement
- Add lightweight profiling logs (optional) and benchmark-like tests for hot functions
- Optimize re-renders and redundant invokes
- Backend: move blocking work to `spawn_blocking`, add indexes

Phase 4 — Hardening
- Add edge-case tests (paths with Unicode, missing files, permission errors)
- Add concurrency tests for tracker/process management where feasible

## 8) Verification Approach

### Frontend
- `pnpm test` (to be added)
- `pnpm build` (ensures TS + bundling)

### Backend
- `cargo test` (run in `src-tauri`)
- `cargo fmt --check` and `cargo clippy -- -D warnings` (recommended)

### Cross-layer
- Add a small “contract suite” ensuring TS invoke names match Rust command exports.

