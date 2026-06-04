# Evidence — Raycast compatibility runtime foundation

Verified at: 2026-06-04T16:05:28+03:00

## Summary

All acceptance criteria are `PASS`.

## Results

### AC1 — `@raycast/api` shim exports Phase 1 surface

Verdict: `PASS`

Evidence:

- Added private workspace package `packages/raycast-api` named `@raycast/api`.
- Exports runtime-backed APIs and serializable command primitives from `packages/raycast-api/src/index.ts`.
- `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 15 pass, 0 fail.

### AC2 — Raycast package manifests parse

Verdict: `PASS`

Evidence:

- Added parser in `shell/electron/raycast/manifest.ts`.
- Unit coverage: `tests/unit/raycast-manifest.test.ts`.
- Command result: same `bun test` run above, 15 pass, 0 fail.

### AC3 — Extension discovery recognizes Raycast package manifests

Verdict: `PASS`

Evidence:

- `shell/electron/extension-host.ts` now accepts either `manifest.json` or Raycast `package.json`.
- Raycast commands are mapped to declared commands with ids `${extensionId}:${commandName}`.
- `bun run shell:typecheck`
  - Result: `tsc --noEmit` passed.

### AC4 — Raycast no-view runner executes with runtime-backed APIs

Verdict: `PASS`

Evidence:

- Added `shell/electron/raycast/command-runner.ts`.
- Unit coverage writes and reads runtime-backed LocalStorage/Cache, captures Clipboard, feedback, preferences, and `LaunchType.UserInitiated`.
- Fixture command imports bare `@raycast/api`; runner rewrites it to a host-provided runtime bridge so extension code does not need its own local `node_modules/@raycast/api`.
- Command result: same `bun test` run above, 15 pass, 0 fail.

### AC5 — User-installed Raycast JS execution is blocked

Verdict: `PASS`

Evidence:

- `runRaycastNoViewCommand` refuses `source: "user"` before loading command code.
- Unit coverage: `tests/unit/raycast-command-runner.test.ts`.
- Existing permission regression coverage was also rerun: `tests/unit/extension-permissions.test.ts`.

### AC6 — Existing extension behavior remains compatible

Verdict: `PASS`

Evidence:

- `bun run shell:typecheck`
  - Result: passed.
- `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 15 pass, 0 fail.
- `bun run ark:guard:writes`
  - Result: `ARK write boundary guard passed.`

### AC7 — Full verification commands

Verdict: `PASS`

Evidence:

- `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 15 pass, 0 fail.
- `bun run shell:typecheck`
  - Result: passed.
- `bun run docs:sync`
  - Result: regenerated AGENTS/CLAUDE/llms artifacts successfully.
- `bun run docs:check`
  - Result: docs fresh.
- `bun run ark:guard:writes`
  - Result: passed.
- `bun run ark:smoke`
  - First run: timed out at 120s during shell/extensions build; output showed an EPIPE after the command was interrupted.
  - Re-run with 600s timeout: `ARK smoke matrix passed.`

## Notes

- Several commands initially failed before execution with `windows sandbox: setup refresh failed with status exit code: 1`; per `windows-sandbox` skill, they were rerun with scoped escalation.
- No visual verification was required: this change adds runtime/parser/API foundation and no visible renderer surface.
