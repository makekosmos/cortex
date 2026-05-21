# Evidence: Arrancador library toolbar icon layout

## Result

PASS

## Acceptance Criteria

- AC1: PASS. The library header now renders icon-only filter and sort buttons with accessible labels. See `apps/arrancador/src/pages/Library.tsx:883` and `apps/arrancador/src/pages/Library.tsx:899`.
- AC2: PASS. The view mode toggle is centered in the top toolbar area. See `apps/arrancador/src/pages/Library.tsx:924`.
- AC3: PASS. `Избранное` and `Сбросить` were removed from the top toolbar and moved into the expanded filters panel. See `apps/arrancador/src/pages/Library.tsx:963` and `apps/arrancador/src/pages/Library.tsx:973`.
- AC4: PASS. The updated library toolbar compiles and the targeted library page test suite passes. See `apps/arrancador/src/test/library.test.tsx:104-152`, `raw/typecheck.txt`, and `raw/library-vitest.txt`.

## Verification

- `bun run typecheck`
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/library.test.tsx`

## Raw Artifacts

- `raw/typecheck.txt`
- `raw/library-vitest.txt`
