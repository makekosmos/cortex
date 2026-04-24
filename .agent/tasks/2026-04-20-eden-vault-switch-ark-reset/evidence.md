# Evidence

## Result

PASS

## Acceptance Criteria

- AC1 PASS: `setVaultPath()` now calls `shutdownArk()` after updating shared selected space in `apps/eden/ts/main/store.ts`.
- AC2 PASS: `apps/eden/ts/main/ark.ts` already resolves DB path from current shared selected space, so the next Ark request re-initializes against the new DB after reset.
- AC3 PASS: Fallback behavior remains unchanged when shared selected space is absent.
- AC4 PASS: `bun run build` in `apps/eden/ts` passed.
