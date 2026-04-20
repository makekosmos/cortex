# Evidence: Vue Settings Page Migration

## Verification Status

- Acceptance criteria status: `PASS` for AC1-AC8.
- Command verification status: `BLOCKED` by an existing Vite/Tailwind native-binding load failure during renderer build config loading.

## Changed Files

- `src-vue/router.ts`
- `src-vue/pages/SettingsPage.vue`
- `src-vue/composables/useSettingsPageState.ts`
- `src-vue/components/settings/types.ts`
- `src-vue/components/settings/SettingsSectionNav.vue`
- `src-vue/components/settings/SettingsToggleRow.vue`
- `src-vue/components/settings/AppearanceSettingsSection.vue`
- `src-vue/components/settings/SystemSettingsSection.vue`
- `src-vue/components/settings/BackupSettingsSection.vue`
- `src-vue/components/settings/CompressionSettingsSection.vue`
- `src-vue/components/settings/SqobaSettingsSection.vue`
- `src-vue/components/settings/RawgSettingsSection.vue`

## Acceptance Criteria

- AC1 `PASS`
  - `src-vue/router.ts` now routes `/settings` to `SettingsPage`.
- AC2 `PASS`
  - `src-vue/composables/useSettingsPageState.ts` loads settings with `settingsApi.getAll()` and autostart with `getAutoStartState()`, then hydrates the editable Vue form state.
- AC3 `PASS`
  - `src-vue/pages/SettingsPage.vue` composes sections for theme, system autostart, backups, compression, SQOBA, and RAWG.
- AC4 `PASS`
  - `src-vue/composables/useSettingsPageState.ts` uses `pickDirectoryPath()` via `selectBackupDirectory()`.
- AC5 `PASS`
  - `saveSettings()` in `src-vue/composables/useSettingsPageState.ts` persists settings through `settingsApi.update()`, calls `metadataApi.setApiKey()` when the RAWG key changes, reloads canonical state, and exposes save progress/feedback.
- AC6 `PASS`
  - `refreshSqobaManifest()` in `src-vue/composables/useSettingsPageState.ts` drives in-progress and success/error feedback consumed by both backup and SQOBA sections.
- AC7 `PASS`
  - Production edits are limited to `apps/arrancador/src-vue/**`.
- AC8 `PASS`
  - All new Vue code uses Composition API with `<script setup lang="ts">`; the route view stays as a composition surface with child section components and the existing Tailwind-based styling approach.

## Command Evidence

- `bun x tsc --noEmit --pretty false`
  - Result: `PASS`
  - Artifact: `raw/tsc-noemit.txt`
- `bun run build:renderer:vue`
  - Result: `FAIL`
  - Artifact: `raw/build-renderer-vue.txt`
  - Failure source: Vite config load aborts before app bundling because `@tailwindcss/oxide-win32-x64-msvc` cannot be loaded and the config path also reports `spawn EPERM`.
- `node .\\node_modules\\vite\\bin\\vite.js build`
  - Result: `FAIL`
  - Artifact: `raw/build-renderer-vue-node.txt`
  - Failure source: same config-load/native-binding issue, confirming the blocker is not specific to `bun run`.

## Notes

- The existing React behavior for theme switching is mirrored through the Vue theme composable and remains immediate client-side UI state.
- No React source files under `src/` were modified.
