# Evidence: Eden Memory Leak Fixes

## Result

Verification status: `FAIL`

Acceptance criteria status:

- `AC1` `PASS` — renderer perf samples are bounded in `apps/eden/ts/src/Editor.vue`.
- `AC2` `PASS` — sidecar stderr retention is bounded in `apps/eden/ts/main/heart.ts` and `apps/eden/ts/main/ark.ts`.
- `AC3` `PASS` — transient save bookkeeping is pruned and released in `apps/eden/ts/src/store/eden.ts`.
- `AC4` `PASS` — navigation history is capped to the latest 30 actions in `apps/eden/ts/src/App.vue`.
- `AC5` `PASS` — Hevy login timers are tracked and cleared in `apps/eden/ts/main/hevy.ts`.

Overall verification is `FAIL` only because the required fresh Playwright pass could not start a worker process in the current environment (`spawn EPERM`).

## Code Evidence

- Bounded perf samples:
  - `apps/eden/ts/src/Editor.vue:148`
  - `apps/eden/ts/src/Editor.vue:401`
  - `apps/eden/ts/src/Editor.vue:405`
- Bounded sidecar stderr tails:
  - `apps/eden/ts/main/heart.ts:31`
  - `apps/eden/ts/main/heart.ts:99`
  - `apps/eden/ts/main/ark.ts:32`
  - `apps/eden/ts/main/ark.ts:125`
- Save bookkeeping cleanup:
  - `apps/eden/ts/src/store/eden.ts:55`
  - `apps/eden/ts/src/store/eden.ts:101`
  - `apps/eden/ts/src/store/eden.ts:417`
  - `apps/eden/ts/src/store/eden.ts:488`
- Navigation history cap:
  - `apps/eden/ts/src/App.vue:367`
  - `apps/eden/ts/src/App.vue:379`
  - `apps/eden/ts/src/App.vue:530`
  - `apps/eden/ts/src/App.vue:560`
- Hevy timer cleanup:
  - `apps/eden/ts/main/hevy.ts:51`
  - `apps/eden/ts/main/hevy.ts:58`
  - `apps/eden/ts/main/hevy.ts:104`
  - `apps/eden/ts/main/hevy.ts:132`

## Command Evidence

- `bun run lint` — `PASS`
- `bun run build` — `PASS`
- `bun run test:e2e` — `FAIL` due environment-level `spawn EPERM`

Raw artifacts:

- `.agent/tasks/2026-04-21-eden-memory-leak-fixes/raw/lint.txt`
- `.agent/tasks/2026-04-21-eden-memory-leak-fixes/raw/build.txt`
- `.agent/tasks/2026-04-21-eden-memory-leak-fixes/raw/test-e2e.txt`
