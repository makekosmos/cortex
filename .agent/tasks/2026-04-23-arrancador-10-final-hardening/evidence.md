# Evidence

## Summary

Verification result: PASS

This pass removes the main issues that capped the previous review score: duplicated IPC runtime contract state, oversized route-level Vue files, and missing tests for the new boundaries.

## Acceptance Criteria

### AC1: IPC runtime contract is single-source

PASS

- Replaced independent IPC command/no-arg/event lists in `apps/arrancador/electron/shared/ipc.ts` with typed `IPC_COMMAND_REGISTRY` and `IPC_EVENT_REGISTRY`.
- Allowed command channels, no-arg behavior, and allowed event channels are now derived from those registries.
- Existing payload validators remain active.

### AC2: IPC contract behavior is tested

PASS

- Added `apps/arrancador/electron/main/shared-ipc.test.ts`.
- Covers known commands, unknown invoke blocking, no-arg payload rejection, malformed payload rejection, and unknown event blocking.

### AC3: Oversized Vue route code is atomized

PASS

- Added `apps/arrancador/src-vue/components/scan/ScanResultsList.vue`.
- Added `apps/arrancador/src-vue/components/game-detail/GameDetailStateSection.vue`.
- `ScanPage.vue`: 563 lines.
- `GameDetailPage.vue`: 629 lines.
- No active route-level Vue file in `apps/arrancador/src-vue/pages` exceeds 700 lines.

### AC4: New Vue component boundaries are tested

PASS

- Added `apps/arrancador/src-vue/test/scan-results-list.test.ts`.
- Extended `apps/arrancador/src-vue/test/game-detail-components.test.ts` for `GameDetailStateSection`.
- Tests cover emitted event contracts and model-update contracts.

### AC5: Existing behavior remains green

PASS

Fresh verification from `apps/arrancador`:

- `bun run typecheck`: PASS.
- `bun run test`: PASS, 22 files / 64 tests.

### AC6: Proof artifacts exist

PASS

Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `metrics.txt`
- `diff.txt`
- `status.txt`
- `problems.md`

## Notes

The first verification pass failed on test expectations and encoding guard issues in the newly extracted components. `problems.md` records the failures and fixes. The final fresh verification pass is PASS.
