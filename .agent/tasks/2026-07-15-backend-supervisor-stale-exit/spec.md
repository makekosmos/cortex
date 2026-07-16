# Backend supervisor stale-exit recovery

## Goal

Prevent a late exit event from a replaced backend process from clearing the active replacement and scheduling duplicate respawns; keep normal dev sessions from colliding with the installed LAN sync listener.

## Scope

In scope:

- process-identity fencing in the Electron backend supervisor;
- per-process runtime duration accounting;
- default `KEPLER_SKIP_SYNC=1` for `scripts/dev.mjs`, while preserving an explicit environment override;
- regression tests and postmortem documentation.

Out of scope:

- changing production LAN sync port allocation, killing installed Kosmos processes, or suppressing genuine backend crashes.

## Acceptance criteria

**AC1.** After backend A is replaced by backend B, a late `exit` from A does not clear B, reset B's ArkClient, or schedule a respawn.

**AC2.** An exit from the current backend still clears the active process, resets ArkClient, and follows the existing bounded respawn policy.

**AC3.** Standard `scripts/dev.mjs` launches the shell with `KEPLER_SKIP_SYNC=1` unless the caller explicitly supplied another value.

**AC4.** Targeted Bun tests, desktop typecheck/build, docs sync/check, formatting, and a runtime smoke check pass against the final worktree.
