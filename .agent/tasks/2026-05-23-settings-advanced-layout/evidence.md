# Evidence - Kepler settings advanced layout

## Verification Date

- 2026-05-23

## Results

- AC1 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`.
  - `Фокус` now has `group: "advanced"` and renders with the advanced navigation items.

- AC2 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`.
  - The content header no longer renders the active page title.

- AC3 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`.
  - The content header no longer applies `border-bottom`.

- AC4 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`, `shell/electron/settings-window.ts`, `shell/electron/preload.ts`, and `shell/shared/ipc-types.ts`.
  - Basic pages render back, forward, minimize, and close controls. Maximize/restore is not rendered. Minimize is now backed by `kepler:settings:minimize`.

- AC5 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`.
  - Advanced pages render the same controls plus a disabled gray switch placeholder.

- AC6 — PASS
  - Verified by code inspection in `shell/src/views/SettingsView.vue`.
  - Search and active-tab data loading still use the same computed `activeTab` and watcher flow.

- AC7 — PASS
  - Command: `bun run --cwd shell typecheck`
  - Log: `.agent/tasks/2026-05-23-settings-advanced-layout/raw/shell-typecheck.txt`

- AC7 — PASS
  - Command: `bun run --cwd shell build:js`
  - Log: `.agent/tasks/2026-05-23-settings-advanced-layout/raw/shell-build-js.txt`
