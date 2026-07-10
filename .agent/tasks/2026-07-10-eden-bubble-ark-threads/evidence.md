# Independent verifier evidence

Verdict: **PASS**

Reverified after the task-fixer against `C:\Users\kirill\Coding\kosmos` on 2026-07-10. The ambient `D:\Personal\Hobby\Coding\kosmos` path does not exist on this machine. All runtime and visual fixtures used task-local or in-memory data; no production/user ARK data was opened.

## Fresh commands

| Command                                                                                                                                                                                                  | Outcome                                                             |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `rtk bun test tests/bubbleDiaryModel.test.ts tests/bubbleArkApi.test.ts tests/liveListFilter.test.ts` (cwd `products/eden`)                                                                              | PASS, 28 tests                                                      |
| `rtk powershell -NoProfile -Command "rtk bun test (Get-ChildItem -LiteralPath 'tests' -Filter '*.test.ts' \| ForEach-Object FullName)"` (cwd `products/eden`)                                            | PASS, 94 tests                                                      |
| `rtk bunx vitest run --config vitest.config.ts tests/components/BubbleDiaryView.spec.ts` (cwd `products/eden`)                                                                                           | PASS, 7 tests                                                       |
| `rtk bun run test:vue` (cwd `products/eden`)                                                                                                                                                             | PASS, 71 tests across 14 files                                      |
| `rtk node platform/desktop/node_modules/typescript/bin/tsc --noEmit -p .agent/tasks/2026-07-10-eden-bubble-ark-threads/smoke/tsconfig.eden-bubble.json --typeRoots platform/desktop/node_modules/@types` | PASS, strict bubble/Eden TS surface                                 |
| `rtk bun run --cwd core/ark/packages/ark typecheck`                                                                                                                                                      | PASS, exact package typecheck                                       |
| `rtk test bun run build:extension eden` (cwd `platform/desktop`)                                                                                                                                         | PASS                                                                |
| `rtk bun run ark:guard:writes`                                                                                                                                                                           | PASS                                                                |
| `rtk bun .agent/tasks/2026-07-10-eden-bubble-ark-threads/smoke/ark-bubble-runtime-smoke.ts`                                                                                                              | PASS, real sidecar migration/retry/restart/link/delete/detach smoke |
| `$env:ARK_SMOKE_TASK_ID='2026-07-10-eden-bubble-ark-threads'; rtk bun run ark:smoke`                                                                                                                     | PASS, complete smoke matrix                                         |
| `$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; rtk node .agent/tasks/2026-07-10-eden-bubble-ark-threads/smoke/visual-eden-smoke.mjs`                                                              | PASS on port 9912, two PNGs captured                                |
| `rtk git diff --check`                                                                                                                                                                                   | PASS                                                                |

Vitest still emits the repository's existing `ResizeObserver loop completed with undelivered notifications` console noise, but both focused and full browser runs exit 0 with all assertions passing.

The first fresh full `ark:smoke` attempt hit one unrelated intermittent dictation temp-path test (`local_models_use_parakeet_directory_without_whisper_command`: 430 passed, 1 failed). That exact test immediately passed alone, and a second unmodified full `ark:smoke` run passed the entire matrix. This transient is recorded here rather than hidden; it did not reproduce and is outside the bubble implementation.

## Real ARK runtime proof

`smoke/ark-bubble-runtime-smoke.ts` built/used the current `target/debug/ark-core-rpc.exe` with the isolated task DB `smoke/ark-runtime/ark.db`.

- Wrote a canonical `system-type-journal` root, child bubble, and deterministic `reply_to` link.
- Updated body, tags, kind, and preserved occurrence metadata.
- Migrated rich dated input and read it back using semantic JSON equality.
- Retried migration and obtained the same deterministic ID without duplication.
- Restarted the sidecar and reconstructed the reply beneath the same root.
- Deleted the root, restarted again, and observed the surviving child as a root with zero remaining links.

Fresh summary:

```json
{
  "status": "PASS",
  "migratedId": "eden-bubble-f81f0d43dba45ff4dfc2fc46a2b2326e",
  "finalLinks": 0,
  "migrationReadBackPassed": true
}
```

## Mounted midnight and event proof

The focused browser test now mounts the actual diary component and proves:

- timer rollover changes `14:05` to `Вчера, 14:05` without an ARK write;
- `focus` and visible `visibilitychange` recompute labels after suspended time;
- unmount clears the timer/listeners;
- `object_upserted` adds the new bubble;
- `object_deleted` removes it;
- direct current payload `entity_changed { entity_type: "object_link" }` regroups the child as a reply;
- subscriptions are removed on unmount and grouping survives remount/read-back.

No polling or new event/ARK endpoint was added.

## Visual proof

The visual smoke started the actual Eden Vite dev surface on `127.0.0.1:9912` with `KOSMOS_HEADLESS=1` and `KOSMOS_TEST_MODE=1`, using an in-memory ARK fixture.

- `smoke/visual/today-yesterday-older.png` was inspected and simultaneously shows `14:02`, `Вчера, 16:30`, and `2 июл, 09:05`.
- `smoke/visual/one-level-thread.png` was inspected and shows the child below the root with a visible L-shaped rail/connector. The child has no nested reply action.

No process remained listening on port 9912 after capture.

## Previously reported problems

- AC5 midnight/focus proof: **RESOLVED** by the mounted fixed-clock browser test.
- AC7 visual connector proof: **RESOLVED** by the two inspected headless screenshots.
- AC9/AC10 migration mismatch: **RESOLVED** by stable semantic JSON comparison, regression test, and real sidecar smoke.
- AC11 event proof: **RESOLVED** by mounted object upsert/delete and direct object-link entity event tests.
- AC12 compiler/smoke environment: **RESOLVED** by ignored junctions to the already-installed pinned TypeScript and Node types; lockfiles stayed clean; exact package typecheck and full smoke pass.
- AC13 incomplete bundle/typecheck: **RESOLVED** by fresh strict TS, full tests/build/smokes, and visual artifacts.

Per task-verifier protocol, `problems.md` was removed because no non-PASS criterion remains; this section preserves the prior failure history.

## Acceptance criteria

| AC   | Status | Fresh evidence                                                                    |
| ---- | ------ | --------------------------------------------------------------------------------- |
| AC1  | PASS   | Canonical object/link runtime smoke; bubble/API/list-filter tests.                |
| AC2  | PASS   | ARK-only component load, deterministic ordering tests, real restart/read-back.    |
| AC3  | PASS   | Focused CRUD tests and real ARK update/delete proof.                              |
| AC4  | PASS   | Fixed-clock local-calendar boundary tests.                                        |
| AC5  | PASS   | Mounted midnight timer, focus, visibility, no-write, and cleanup test.            |
| AC6  | PASS   | Real durable reply link/restart proof and deterministic link behavior.            |
| AC7  | PASS   | One-level normalization/browser tests plus inspected connector screenshot.        |
| AC8  | PASS   | Root/child delete tests and real root-delete/restart/detach proof.                |
| AC9  | PASS   | Semantic read-back regression plus real idempotent migration smoke.               |
| AC10 | PASS   | Ambiguous-input preservation test and verified read-back-before-cleanup flow.     |
| AC11 | PASS   | Mounted current object upsert/delete and direct object-link entity event proof.   |
| AC12 | PASS   | Existing boundary only; guard, exact SDK typecheck, and full `ark:smoke` pass.    |
| AC13 | PASS   | Focused/full tests, strict TS, build, isolated ARK smoke, and two inspected PNGs. |
