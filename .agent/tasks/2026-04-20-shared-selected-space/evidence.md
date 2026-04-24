# Evidence

## Result

PASS

## Acceptance Criteria

- AC1 PASS: Added shared helper at `packages/shared-space/selectedSpace.ts` that reads/writes `appData/Kepler/selected-space.json`.
- AC2 PASS: `Eden` now writes shared selected space on `setVaultPath()` and resolves Ark DB path from shared selection in `apps/eden/ts/main/store.ts` and `apps/eden/ts/main/ark.ts`.
- AC3 PASS: `Arrancador` now resolves Ark DB path from shared selected space in `apps/arrancador/electron/main/backend.ts`.
- AC4 PASS: `Delphi Electron` now reads shared selected space on startup, switches sidecar DB to that `spaceId`, and syncs `space:setActive` with the shared selection in `apps/delphi/ts/electron/main.ts`.
- AC5 PASS: All touched paths still fall back to legacy `appData/Kepler/ark.db` or local config when shared selection is absent.
- AC6 PASS: Added code uses ASCII-only strings except existing app strings; grep for mojibake markers in touched files returned no matches.

## Verification

- `bun run build` in `apps/eden/ts` passed.
- `bunx tsc --noEmit` in `apps/delphi/ts` passed.
- `bunx tsc --noEmit` in `apps/arrancador` passed.
- `bun run build:main` in `apps/arrancador` passed.
- Deterministic helper check passed: the same vault path produced the same `spaceCode` and `spaceId` twice.

## Notes

- Full `bun run build` in `apps/delphi/ts` reaches `electron-builder` and fails in existing packaging dependencies (`ajv-keywords` / `electron-builder`), after Vite and Electron main bundles were already built successfully.
- Full `bun run build` in `apps/arrancador` fails in existing environment-native build issues (`tailwindcss-oxide` / `spawn EPERM`) unrelated to this change set.
