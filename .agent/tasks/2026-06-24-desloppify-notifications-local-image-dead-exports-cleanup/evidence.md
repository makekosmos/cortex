# desloppify notifications + local image dead exports cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-knip-test-entry-autostart-cleanup/desloppify-after.json`
- score 9, total 251, HIGH 98, MEDIUM 109, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-notifications-local-image-dead-exports-cleanup/desloppify-after.json`
- score 9, total 249, HIGH 96, MEDIUM 109, LOW 44

Rule delta:

- `DEAD_EXPORT`: 43 -> 41

Checks:

- `bun test platform/desktop/electron/local-image-protocol.test.ts platform/desktop/electron/settings-autostart.test.ts`
- `bun run --cwd platform/desktop typecheck`
- mojibake/no-op grep over touched files

Rejected config experiment:

- Adding `site/src/main.ts` and `site/vite.config.ts` as root Knip entries reduced `DEAD_FILE` but added `UNLISTED_DEPENDENCY` findings for site-only Vite deps and dropped score 9 -> 8.
- The site entry change was removed from `knip.json`.
