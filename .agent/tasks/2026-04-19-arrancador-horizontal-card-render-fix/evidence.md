# Evidence - Arrancador horizontal card render fix

## Scope
- `packages/kepler-visuals/patterns/gamePosterCard.ts`
- `packages/kepler-visuals/theme/css-variables.css`
- `.agent/tasks/2026-04-19-arrancador-horizontal-card-render-fix/spec.md`

## What changed
- Removed the critical `aspect-[16/9]` dependency from the shared card recipe in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/patterns/gamePosterCard.ts:1).
- Moved the horizontal card geometry into stable shared CSS by setting `aspect-ratio: 16 / 9` on `.kepler-game-poster-card` in [css-variables.css](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/theme/css-variables.css:90).
- Moved the lower scrim height/gradient and title clamping into the same shared CSS file so the card no longer depends on fragile arbitrary utility classes from the external package.

## Acceptance criteria
- `AC1` PASS: shared game cards now have an explicit CSS `aspect-ratio: 16 / 9`, so they retain visible height even if external-package arbitrary Tailwind utilities are not emitted.
- `AC2` PASS: card height no longer depends on `aspect-[16/9]`; the shared recipe in [gamePosterCard.ts](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/patterns/gamePosterCard.ts:1) no longer includes that class.
- `AC3` PASS: the bottom scrim and title layer remain defined via `.kepler-game-poster-card__scrim`, `.kepler-game-poster-card__eyebrow`, and `.kepler-game-poster-card__title` in [css-variables.css](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/theme/css-variables.css:98).
- `AC4` PASS: `bunx tsc --noEmit` and `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` both pass.
- `AC5` PASS: no mojibake was introduced in changed files; a targeted replacement-character search returned no matches.

## Verification
### Fresh commands
- `rg --line-number "aspect-\[16/9\]|h-\[62%\]|line-clamp-2" .\packages\kepler-visuals .\apps\arrancador` PASS
- `bunx tsc --noEmit` PASS
- `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` PASS
- `rg --line-number "�" .\packages\kepler-visuals\theme\css-variables.css .\packages\kepler-visuals\patterns\gamePosterCard.ts .\.agent\tasks\2026-04-19-arrancador-horizontal-card-render-fix` PASS

### Raw artifacts
- `.agent/tasks/2026-04-19-arrancador-horizontal-card-render-fix/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-horizontal-card-render-fix/game-card-vitest.log`

## Notes
- I attempted a live Playwright screenshot against the running Vite server, but the local sandbox blocks headless Chromium launch with `spawn EPERM`, so runtime visual confirmation was unavailable from tooling.
- The fix targets the most likely root cause of the disappearing cards: zero-height links caused by absolute-positioned children plus a missing generated `aspect-*` utility.
