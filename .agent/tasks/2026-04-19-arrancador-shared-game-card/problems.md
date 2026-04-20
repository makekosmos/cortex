# Problems - Arrancador shared game card via kepler-visuals

## P1: Shared Tailwind classes were not included in Arrancador's content scan
- Symptom: the hover darkening change in `packages/kepler-visuals/patterns/gamePosterCard.ts` did not appear in the running Arrancador UI.
- Root cause: [tailwind.config.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/tailwind.config.ts:1) only scanned `./index.html` and `./src/**/*.{ts,tsx}`. Shared utility strings moved into `packages/kepler-visuals` were therefore outside Tailwind's `content` globs, so the renderer build did not reliably emit the required classes.
- Fix: added `../../packages/kepler-visuals/**/*.{ts,tsx}` to Arrancador's Tailwind `content` list.
- Verification: `bun run build:renderer`, `bun run typecheck`, and `bunx vitest run --configLoader native --config vitest.config.mjs src/test/game-card.test.tsx` all pass after the config update.

## Operational note
- Because this fix touches Tailwind configuration, an already running `bun run dev` process must be restarted to pick up the new content glob.
