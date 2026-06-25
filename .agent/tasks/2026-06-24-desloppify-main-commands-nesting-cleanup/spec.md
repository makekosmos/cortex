# Main Commands Nesting Cleanup

Goal: remove the last `DEEP_NESTING` finding in `platform/desktop/electron/main-commands.ts`
without changing command merge behavior.

Change: extract the runtime dynamic command merge into `mergeDynamicCommands(byId, dynamic)`.

Verification:

- `rtk err bun run --cwd platform/desktop typecheck`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-main-commands-nesting-cleanup.json"`
