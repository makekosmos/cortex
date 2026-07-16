# Evidence — backend supervisor stale-exit recovery

## Result

All acceptance criteria pass on the final worktree.

| AC  | Evidence                                                                                                                                                                                                                                                            | Result |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| AC1 | `main-backend-supervisor.test.ts` replaces fake backend A with B, emits A's late `exit`, and asserts B remains active with no second ArkClient reset. The regression test failed before the identity fence (`Expected true`, `Received false`) and passes after it. | PASS   |
| AC2 | The second supervisor test emits `exit` from the current backend and asserts that the active slot is cleared and ArkClient is reset once.                                                                                                                           | PASS   |
| AC3 | `platform/desktop/scripts/dev.mjs` sets `KEPLER_SKIP_SYNC` to `process.env.KEPLER_SKIP_SYNC ?? "1"`, so normal dev avoids the installed instance's fixed port while an explicit `0` is preserved.                                                                   | PASS   |
| AC4 | Targeted Bun tests, desktop typecheck and shell build, docs sync/check, oxfmt, launcher calculator E2E smoke, and `git diff --check` pass.                                                                                                                          | PASS   |

## Runtime diagnosis

- The LAN sync bind warning was real: TCP port `21531` was already owned by PID `21084`, the installed `Kosmos Data Engine.exe` under `AppData\Local\Programs\kepler-shell`.
- Whisper model/buffer initialization messages and optional `ggml_backend_init` probing were informational; STT preload completed successfully.
- The actionable supervisor failure was the late `exit` callback of a replaced child process mutating the shared active-process slot.

## Verification commands

```powershell
bun test platform/desktop/electron/main-backend-supervisor.test.ts
bun run --cwd platform/desktop typecheck
bun run --cwd platform/desktop build:js:shell
bun run docs:sync
bun run docs:check
bunx oxfmt --check platform/desktop/electron/main-backend-supervisor.ts platform/desktop/electron/main-backend-supervisor.test.ts platform/desktop/scripts/dev.mjs docs-site/agents/postmortems.md
bunx playwright test tests/e2e/launcher-calculator.spec.ts --config playwright.config.ts
git diff --check
```
