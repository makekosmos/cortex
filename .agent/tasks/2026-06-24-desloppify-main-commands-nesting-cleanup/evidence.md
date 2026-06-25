# Evidence

- Baseline copied to `.agent/tasks/2026-06-24-desloppify-main-commands-nesting-cleanup/baseline/desloppify-before.json`
- Runtime merge extracted to `mergeDynamicCommands(byId, dynamic)` in `platform/desktop/electron/main-commands.ts`
- `rtk err bun run --cwd platform/desktop typecheck` passed
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-main-commands-nesting-cleanup.json"` produced the after scan
- `DEEP_NESTING` for `platform/desktop/electron/main-commands.ts` went from 1 to 0
- Total high findings went from 143 to 142
