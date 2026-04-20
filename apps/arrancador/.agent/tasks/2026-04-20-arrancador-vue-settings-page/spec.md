# Task Spec: Vue Settings Page Migration

## Original Task

You are worker A on the Arrancador React->Vue/Vapor migration in `D:\Personal\Hobby\Coding\kepler\apps\arrancador`. Ownership: create Vue settings composable/components/page only under `apps/arrancador/src-vue/**`, and if necessary add minimal supporting types/imports in that same tree. Do not touch `src/` React files. You are not alone in the codebase; do not revert others' edits, and adapt to concurrent changes. Task: port the current React settings experience into Vue using Composition API `<script setup lang="ts">`. Reuse existing framework-agnostic APIs from `src/lib/api.ts` and behavior from `src/hooks/useSettingsState.ts` where useful. Aim to deliver a real `SettingsPage.vue` plus any composable(s) it needs, wired for theme switching, backup settings, compression settings, SQOBA manifest refresh, RAWG key, and save flow. Keep UI pragmatic and existing Tailwind look. At the end, report changed files and any gaps; edit files directly in your workspace.

## Scope

Port the current React settings experience into the Vue/Vapor renderer by replacing the Vue settings placeholder route with a real settings page, backed by a Vue composable that mirrors the existing data-loading, update, autostart, backup, compression, SQOBA refresh, RAWG key, and save behaviors.

## Component Map

- `pages/SettingsPage.vue`
  - Route-level composition surface for settings sections, section navigation, and sticky save actions.
- `composables/useSettingsPageState.ts`
  - Owns settings loading, local editable state, autostart toggling, save flow, manifest refresh, derived dirty/disabled state, and toast/status feedback.
- `components/settings/SettingsSectionNav.vue`
  - Renders section chips and emits scroll requests.
- `components/settings/AppearanceSettingsSection.vue`
  - Theme selection UI using the shared Vue theme composable.
- `components/settings/SystemSettingsSection.vue`
  - Autostart toggle UI.
- `components/settings/BackupSettingsSection.vue`
  - Backup directory, max backups, auto-backup, backup-before-launch, and manifest refresh UI.
- `components/settings/CompressionSettingsSection.vue`
  - Compression enablement, level, slider, and skip-once UI.
- `components/settings/SqobaSettingsSection.vue`
  - SQOBA manager entry point and manifest refresh affordance.
- `components/settings/RawgSettingsSection.vue`
  - RAWG API key input and external docs link.

## Acceptance Criteria

- AC1: `/settings` in the Vue router renders a real `SettingsPage.vue` instead of the migration placeholder.
- AC2: The Vue settings page loads current app settings with `settingsApi.getAll()` and autostart state with the existing browser bridge helper, then hydrates editable form state matching the current React settings experience.
- AC3: The page exposes editable controls for theme switching, autostart, backup directory, auto-backup, backup-before-launch, max backups per game, compression enablement, compression level, skip-compression-once, SQOBA manifest refresh, and RAWG API key.
- AC4: The backup directory picker uses the existing directory picker helper and updates the editable state without touching React files.
- AC5: Save flow persists settings through the existing framework-agnostic APIs, updates the RAWG key through `metadataApi.setApiKey()` when changed, reloads canonical settings after save, and exposes save progress/feedback in the Vue UI.
- AC6: SQOBA manifest refresh is invokable from the Vue settings page, shows in-progress state, and exposes success/error feedback.
- AC7: The implementation stays under `apps/arrancador/src-vue/**` except for imports from existing shared code under `src/**`; no React source files are edited.
- AC8: The Vue implementation uses Composition API with `<script setup lang="ts">`, keeps the route page thin via child section components/composable(s), and preserves the existing Tailwind-based look without introducing a new design system.

## Constraints

- Only edit `apps/arrancador/src-vue/**` for production code in this task.
- Do not touch `apps/arrancador/src/**` React implementation files.
- Do not revert or rewrite unrelated concurrent changes elsewhere in the repository.
- Reuse existing shared APIs/helpers from `apps/arrancador/src/lib/**` and types from `apps/arrancador/src/types/**` where practical.
- Keep the UI pragmatic; no broad visual redesign.

## Non-Goals

- Porting the SQOBA page itself.
- Porting shared React UI primitives into Vue.
- Adding backend IPC handlers or changing settings persistence semantics outside the existing shared APIs.
- Broad localization work beyond what is necessary for this settings slice.

## Assumptions

- The current React `useSettingsState` hook is the behavioral baseline for loading, autostart toggling, save, and manifest refresh semantics.
- Theme switching in this migration slice should use the existing Vue theme composable and current client-side theme behavior rather than inventing new persistence rules.
- Minimal Vue-only child components are acceptable under `src-vue/components/settings/` to keep `SettingsPage.vue` focused.

## Verification Plan

1. Run TypeScript verification for Arrancador (`bun run typecheck` from `apps/arrancador`).
2. Run the Vue renderer build (`bun run build:renderer:vue` from `apps/arrancador`).
3. Record command outputs in `.agent/tasks/2026-04-20-arrancador-vue-settings-page/raw/`.
4. Write `evidence.md` and `evidence.json` with AC-by-AC status based on the current code and command results.
