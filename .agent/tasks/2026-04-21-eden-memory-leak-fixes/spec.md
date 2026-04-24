# Task: Eden Memory Leak Fixes

## Scope

Fix the concrete unbounded-memory growth paths previously identified in `apps/eden/ts`:

- renderer perf metric accumulation in `src/Editor.vue`
- sidecar stderr accumulation in `main/heart.ts` and `main/ark.ts`
- stale save timestamp bookkeeping in `src/store/eden.ts`
- unbounded navigation history in `src/App.vue`
- transient Hevy login timers in `main/hevy.ts`

## Acceptance Criteria

### AC1: Perf metrics are bounded

`src/Editor.vue` must stop retaining an unbounded number of timing samples.

- The perf tracker keeps only a fixed-size recent window of samples per metric.
- Long-task aggregation remains bounded and does not store raw long-task entries.
- Existing `window.__edenPerf.getSummary()` behavior still works.

### AC2: Sidecar stderr retention is bounded

`main/heart.ts` and `main/ark.ts` must stop appending unlimited stderr text for the lifetime of the child process.

- stderr retention is capped to a fixed-size recent tail.
- Error reporting still includes recent stderr output when a child exits or fails.
- stdout request/response flow keeps working.

### AC3: Save bookkeeping does not leak per-entry state

`src/store/eden.ts` must release transient save bookkeeping after saves complete or when entries disappear.

- `latestSaveTimestamps` no longer grows forever across historical entry IDs.
- Save coordination still preserves last-write-wins behavior.

### AC4: Navigation history is capped

`src/App.vue` keeps only the latest 30 history actions for backward/forward navigation.

- Back history retains at most 30 snapshots.
- Forward history also remains bounded when populated by navigation.
- Existing back/forward behavior still works.

### AC5: Hevy login timers are cleaned up

`main/hevy.ts` must not leave pending token-extraction timers alive after the login window resolves or closes.

- Timer handles are tracked.
- All pending timers are cleared on resolve/close.
- Login flow behavior remains unchanged otherwise.

## Verification Plan

- `bun run lint`
- `bun run build`
- `bun run test:e2e`
