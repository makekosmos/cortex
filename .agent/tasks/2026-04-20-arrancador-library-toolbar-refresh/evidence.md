# Evidence: Arrancador library toolbar refresh

## Result

PASS

## Acceptance Criteria

- AC1: PASS. The library header no longer renders the standalone search input in the top-right area, and that area now renders compact action buttons instead. See `apps/arrancador/src/pages/Library.tsx:877-913`.
- AC2: PASS. The top-right header area now renders icon buttons for filter and sort, wired to the existing advanced filter toggle and sort radio group. See `apps/arrancador/src/pages/Library.tsx:886` and `apps/arrancador/src/pages/Library.tsx:902`.
- AC3: PASS. The grid/list view toggle moved into the upper content toolbar and still switches view mode. Accessibility labels were added for the icon buttons. See `apps/arrancador/src/pages/Library.tsx:922-944`.
- AC4: PASS. `bun run typecheck` and the targeted library Vitest suite both passed after the toolbar refresh. The library test now mocks `GameCard` so the page interaction test is isolated from `kosmos-visuals` import resolution. See `apps/arrancador/src/test/library.test.tsx:22-26` and `apps/arrancador/src/test/library.test.tsx:104-131`.

## Verification

- `bun run typecheck`
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/library.test.tsx`

## Raw Artifacts

- `raw/typecheck.txt`
- `raw/library-vitest.txt`
