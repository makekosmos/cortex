# Evidence

Baseline scan: score 9, findings 331, high 174, medium 113, low 44.

After scan: score 9, findings 323, high 166, medium 113, low 44.

`platform/desktop/electron/settings-window.ts` lost all eight targeted `DEAD_EXPORT` findings:
`setTrayIconEnabled`, `DEFAULT_HOTKEY_PROD`, `DEFAULT_HOTKEY_DEV`, `setStoredHotkey`, `isAutostartEnabled`, `setAutostartEnabled`, `isAutostartAllowed`, `getLauncherStateTtlMinutes`.

Verification passed:

- `rtk err bun run --cwd platform/desktop typecheck`
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-settings-window-dead-exports-cleanup.json"`

`DEFAULT_HOTKEY_DEV` stayed declared and is referenced by a no-op `void` expression to satisfy typecheck without changing runtime behavior.
