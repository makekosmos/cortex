# Problems

## 1. First Biome pass failed import sorting

- Command: `bun run biome:check`
- Failure: import ordering in `save-locator.ts` and `save-locator.test.ts`.
- Fix: Reordered imports to match Biome's organizer.
- Reverification: `bun run biome:check` passed.
