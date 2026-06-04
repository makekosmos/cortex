# Evidence - Raycast launchCommand lifecycle slice

Verified at: 2026-06-04T19:05:00+03:00

## Summary

PASS. `launchCommand()` now has a shell-backed lifecycle for trusted
Raycast-compatible commands. Command code can invoke the runtime API, the
command runner forwards the request to the host adapter, and `main.ts` resolves
targets through `findDeclaredCommand()` for `raycast-view`, `raycast-no-view`,
and regular `open` commands.

## Acceptance Criteria

- PASS: `@raycast/api` exports `LaunchTypeValue` for shell-side typing.
- PASS: runner launch props default to `LaunchType.UserInitiated`.
- PASS: host-provided launch props are forwarded to command default exports.
- PASS: `launchCommand()` calls from trusted command code reach the host adapter.
- PASS: shell routing forwards launched commands with
  `LaunchType.LaunchCommand`, `arguments`, `fallbackText`, and `context`.
- PASS: user-installed Raycast command code is still refused by the runner.
- PASS: docs updated in `docs-site/concepts/command-bus.md`,
  `docs-site/concepts/extension-host.md`, and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts`
  - 13 tests passed, 0 failed, 57 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 24 tests passed, 0 failed, 83 expectations after the follow-up
    `List.Dropdown` UI slice.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - First attempt hit a Windows file lock on `target/debug/ark-core-rpc.exe`
    from leftover workspace `kepler-backend.exe` / `ark-core-rpc.exe`
    processes. Confirmed exact workspace-target PIDs, stopped only those dev/test
    processes plus the temporary Vite child, then reran successfully.
  - ARK smoke matrix passed.

## Remaining Scope

Cross-extension launch permission prompts, scheduled/background launch lifecycle,
argument prompt UI, and untrusted extension sandboxing remain for later Raycast
compatibility phases.
