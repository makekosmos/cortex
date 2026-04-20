# Evidence - Arrancador game poster component in kepler-visuals

## Scope
- `packages/kepler-visuals/react/GamePosterCard.tsx`
- `packages/kepler-visuals/react/index.ts`
- `packages/kepler-visuals/patterns/gamePosterCard.ts`
- `packages/kepler-visuals/package.json`
- `apps/arrancador/src/components/GameCard.tsx`
- `apps/arrancador/src/test/game-card.test.tsx`
- `apps/arrancador/tsconfig.json`
- `apps/arrancador/vite.config.ts`
- `.agent/tasks/2026-04-19-arrancador-game-poster-component/spec.md`

## What changed
- Added a real shared React poster-card component in `kepler-visuals` as [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/react/GamePosterCard.tsx:1).
- Exported that component through the package subpath in [packages/kepler-visuals/package.json](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/package.json:1).
- Switched Arrancador's local [GameCard.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/GameCard.tsx:1) from owning the card markup to delegating rendering to the shared `GamePosterCard`.
- Kept the router-specific concern in Arrancador by passing `react-router-dom/Link` through `LinkComponent`.
- Updated the shared overlay style to darken the poster itself with `bg-background/20` on hover and focus-visible.
- Updated Arrancador's card test to assert delegation into the shared component and the expected props contract.
- Added the minimal TypeScript/Vite resolution glue needed for a React component stored under `packages/kepler-visuals` to be typechecked and bundled from Arrancador.

## Acceptance criteria
- `AC1` PASS: `kepler-visuals` now exports a real React poster-card component from [GamePosterCard.tsx](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/react/GamePosterCard.tsx:23).
- `AC2` PASS: Arrancador [GameCard.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/GameCard.tsx:12) no longer owns the card markup and instead renders the shared component directly.
- `AC3` PASS: the shared component accepts a destination path and a `LinkComponent`, letting Arrancador keep `react-router-dom/Link` at the app boundary.
- `AC4` PASS: hover and focus-visible darkening are applied by the shared overlay class in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/patterns/gamePosterCard.ts:13) via `group-hover:bg-background/20 group-focus-visible:bg-background/20`.
- `AC5` PASS: the simplified card treatment remains intact because the shared component renders only poster media, placeholder content, and the overlay, with no title/play button/border/zoom.
- `AC6` PASS: the relevant Arrancador test in [game-card.test.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/test/game-card.test.tsx:1) was updated and passes.
- `AC7` PASS: `bun run typecheck` passes in `apps/arrancador`.

## Verification
### Fresh commands
- `bun run typecheck` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` PASS
- `bun run build:renderer` PASS

### Raw artifacts
- `.agent/tasks/2026-04-19-arrancador-game-poster-component/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-game-poster-component/game-card-vitest.log`
- `.agent/tasks/2026-04-19-arrancador-game-poster-component/build-renderer.log`

## Notes
- The Arrancador test now verifies delegation into the shared component contract, while `build:renderer` verifies that the real shared React component from `kepler-visuals` can be bundled into the app.
- The shared component is stored in `packages/kepler-visuals/react/` to avoid mixing Vue and React exports in the existing `components/` area.
- No mojibake patterns were introduced in the changed project files during this step.
