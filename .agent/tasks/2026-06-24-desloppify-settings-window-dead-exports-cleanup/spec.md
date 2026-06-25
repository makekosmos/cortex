# Settings Window Dead Export Cleanup

Scope: `platform/desktop/electron/settings-window.ts`

Goal: remove safe `export` modifiers from internal settings helpers and constants without changing runtime behavior.

Kept internal and de-exported: `setTrayIconEnabled`, `DEFAULT_HOTKEY_PROD`, `DEFAULT_HOTKEY_DEV`, `setStoredHotkey`, `isAutostartEnabled`, `setAutostartEnabled`, `isAutostartAllowed`, `getLauncherStateTtlMinutes`.

Baseline and after scan JSON are stored in this task directory.
