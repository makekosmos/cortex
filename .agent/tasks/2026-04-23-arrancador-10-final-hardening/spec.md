# 2026-04-23 Arrancador 10 final hardening

## Goal

Raise Arrancador from the previous 7-8/10 quality state to a defensible 10/10 pass by removing the highest remaining architecture, readability, atomicity, and testing gaps in the current codebase.

This task defines 10/10 as a measurable repository gate, not a subjective claim of perfection:

- IPC channels/events have one runtime source of truth;
- remaining oversized Vue route surfaces are split into focused components or composables;
- new boundaries are protected by tests;
- verification commands pass against the current codebase;
- evidence is written under this task directory.

## Acceptance Criteria

### AC1: IPC runtime contract is single-source

`electron/shared/ipc.ts` must no longer maintain independent channel allowlists and no-arg allowlists by hand. The active IPC bridge must derive:

- allowed command channels;
- no-argument command channels;
- allowed event channels;

from typed runtime registries in the same module. Existing payload validation behavior must remain intact.

### AC2: IPC contract behavior is tested

Add focused tests proving:

- known commands are exposed;
- unknown commands are blocked;
- no-arg commands reject unexpected payloads;
- payload validation still rejects malformed command payloads;
- unknown events are blocked.

### AC3: Oversized Vue route code is atomized

Reduce the largest active route-level Vue files by extracting focused boundaries while preserving behavior:

- `ScanPage.vue` should move the scan results list UI into a child component with typed props/emits.
- `GameDetailPage.vue` should move at least one repeated/large detail section into a focused child component with typed props/emits.

After the change, no active route-level Vue file in `apps/arrancador/src-vue/pages` should exceed 700 lines.

### AC4: New Vue component boundaries are tested

Add or extend component tests for the extracted Vue boundaries, including emitted event contracts.

### AC5: Existing behavior remains green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`.

### AC6: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-10-final-hardening/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs for verification;
- metrics proving the route-size gate.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
