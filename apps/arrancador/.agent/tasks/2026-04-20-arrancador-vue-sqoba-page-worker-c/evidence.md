# Evidence: Vue SQOBA Route Migration

## Verification Status

- Acceptance criteria status: `PASS` for AC1-AC8.
- Command verification status: `BLOCKED` by the existing Vite/Tailwind config-loader environment failure during renderer build startup.

## Changed Files

- `src-vue/router.ts`
- `src-vue/pages/SqobaPage.vue`
- `src-vue/composables/useSqobaPageState.ts`
- `src-vue/components/sqoba/types.ts`
- `src-vue/components/sqoba/SqobaSettingsPanel.vue`
- `src-vue/components/sqoba/SqobaSavesPanel.vue`
- `src-vue/components/sqoba/SqobaGameCard.vue`
- `src-vue/components/sqoba/SqobaAboutModal.vue`

## Acceptance Criteria

- AC1 `PASS`
  - `src-vue/router.ts` now routes `/sqoba` to `SqobaPage`.
- AC2 `PASS`
  - `src-vue/composables/useSqobaPageState.ts` loads the current game library from the Vue games store and derives filtered game lists from local search and missing-path state.
- AC3 `PASS`
  - `src-vue/pages/SqobaPage.vue` and `src-vue/components/sqoba/SqobaSettingsPanel.vue` expose manifest refresh plus backup/compression settings using the existing Vue settings composable and shared APIs.
- AC4 `PASS`
  - `loadSavePaths()` in `src-vue/composables/useSqobaPageState.ts` calls `backupApi.findGameSavePaths()` and surfaces warning/error feedback.
- AC5 `PASS`
  - `loadSaveFiles()` in `src-vue/composables/useSqobaPageState.ts` calls `backupApi.findGameSaves()`, and `src-vue/components/sqoba/SqobaGameCard.vue` renders the bounded file list, file count, total size, and open-folder action.
- AC6 `PASS`
  - `saveGameSavePath()`, `selectSaveFolder()`, `selectSaveFile()`, and `insertGamePathToken()` in `src-vue/composables/useSqobaPageState.ts` implement inline save-path editing, picker helpers, token insertion, cancel reset, and persistence through the games store update path.
- AC7 `PASS`
  - `src-vue/pages/SqobaPage.vue` and `src-vue/components/sqoba/SqobaAboutModal.vue` expose the SQOBA about modal while preserving the existing Tailwind visual language.
- AC8 `PASS`
  - Production edits are limited to `apps/arrancador/src-vue/**`, and all new Vue files use Composition API with `<script setup lang="ts">`.

## Command Evidence

- `bun run typecheck`
  - Result: `PASS`
  - Artifact: `raw/typecheck.txt`
- `bun run build:renderer:vue`
  - Result: `FAIL`
  - Artifact: `raw/build-renderer-vue.txt`
  - Failure source: Vite config load aborts before app bundling because `@tailwindcss/oxide-win32-x64-msvc` cannot be loaded and the config path reports `spawn EPERM`.
- `node .\\node_modules\\vite\\bin\\vite.js build --configLoader runner`
  - Result: `FAIL`
  - Artifact: `raw/build-renderer-vue-node-runner.txt`
  - Failure source: alternate config loading also fails before bundling, this time with `ReferenceError: require is not defined` while evaluating dependencies from the Vite config path.
- `node .\\node_modules\\vite\\bin\\vite.js build --configLoader native`
  - Result: `FAIL`
  - Artifact: `raw/build-renderer-vue-node-native.txt`
  - Failure source: native config loading cannot execute `vite.config.ts` under this environment because Node reports `ERR_UNKNOWN_FILE_EXTENSION` for `.ts`.

## Notes

- No React source files under `apps/arrancador/src/**` were modified.
- The port is intentionally minimal: it covers the route placeholder replacement, settings surface, manifest refresh, per-game save lookup, inline path editing, about modal, and file list display, but does not chase full React-text parity.
