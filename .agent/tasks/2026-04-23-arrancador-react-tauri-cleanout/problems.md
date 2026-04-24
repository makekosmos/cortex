# Problems

## Verification pass 1

- `bun run test` failed in `src-vue/test/architecture-boundaries.test.ts`.
- Cause: the active-source guard scanned its own test deny-list, so the forbidden React/Tauri strings used by the guard were reported as offenders.
- Fix: exclude test directories from the active-source import scan while keeping dedicated package, config, source extension, and runtime tree checks active.
