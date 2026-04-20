# Task Spec: Vue SQOBA Route Migration

## Original Task

You are worker C on the Arrancador React->Vue/Vapor migration in `D:\Personal\Hobby\Coding\kepler\apps\arrancador`. Ownership: port the SQOBA route under `apps/arrancador/src-vue/**` only, including any supporting components/composables in `src-vue/components/sqoba` or `src-vue/composables` as needed. Do not touch React `src/` files. You are not alone in the codebase; do not revert others' edits, and adapt to concurrent changes. Task: port the current React SQOBA page into Vue using Composition API `<script setup lang="ts">` with the existing APIs from `src/lib/api.ts` and browser helpers. Preserve the core behaviors: manifest refresh, saves lookup, path editing, backup settings surface, about modal, and file list display. Keep the existing Tailwind visual language. At the end, report changed files and any remaining gaps. Edit files directly in your workspace.

## Scope

Replace the Vue SQOBA migration placeholder with a real SQOBA route that mirrors the current React page's core flows: global SQOBA settings/manifest controls, searchable game list, per-game save-path lookup, per-game file listing, inline save-path editing including `{PATHTOGAME}`, and the informational about modal.

## Component Map

- `pages/SqobaPage.vue`
  - Route-level composition surface for SQOBA header, settings panel, saves panel, and about modal state wiring.
- `composables/useSqobaPageState.ts`
  - Owns SQOBA route state and side effects: games loading, filtering, per-game lookup/file-loading state, edit state, save-path persistence, manifest refresh orchestration, and toast feedback.
- `components/sqoba/SqobaSettingsPanel.vue`
  - Renders manifest refresh and backup/compression settings controls backed by the existing Vue settings composable state.
- `components/sqoba/SqobaSavesPanel.vue`
  - Renders search/filter controls, scan-all affordance, and the list of game save cards.
- `components/sqoba/SqobaGameCard.vue`
  - Renders one game's save-path status, candidate paths, edit actions, file lookup results, and file list surface.
- `components/sqoba/SqobaAboutModal.vue`
  - Presents the SQOBA explainer modal and visual info cards.

## Acceptance Criteria

- AC1: `/sqoba` in the Vue router renders a real `SqobaPage.vue` instead of the migration placeholder.
- AC2: The Vue SQOBA route loads and displays the current game library from the existing Vue games store, with client-side search and an "only missing save path" filter.
- AC3: The route exposes SQOBA manifest refresh and backup/compression settings controls using the existing shared APIs/helpers and the existing Vue settings state, without editing React files.
- AC4: The route supports per-game save-path lookup through `backupApi.findGameSavePaths()` and surfaces success/error states plus candidate-path guidance in the UI.
- AC5: The route supports per-game save file listing through `backupApi.findGameSaves()` and displays the resolved save path, file count, total size, and a bounded file list.
- AC6: The route supports inline save-path editing per game, including folder picker, file picker, `{PATHTOGAME}` insertion, open-path actions, clearing/saving via the games store update path, and current-state reset on cancel.
- AC7: The route exposes the SQOBA about modal from the page header and preserves the Tailwind-based visual language of the current migration shell.
- AC8: Production code changes stay under `apps/arrancador/src-vue/**` only, use Vue Composition API with `<script setup lang="ts">`, and keep the route page thin by delegating feature logic to a composable and focused child components.

## Constraints

- Only edit `apps/arrancador/src-vue/**` for production code in this task.
- Do not touch `apps/arrancador/src/**` React implementation files.
- Do not revert unrelated concurrent changes.
- Reuse existing shared APIs/helpers from `apps/arrancador/src/lib/**`, `apps/arrancador/src/types/**`, and `apps/arrancador/src/lib/browser.ts`.
- Preserve the current Tailwind visual language; no design-system rewrite.

## Non-Goals

- Changing backend IPC handlers or backup engine behavior.
- Porting unrelated routes or expanding the SQOBA feature beyond the existing React page's core behavior.
- Rewriting the shared Vue settings page.

## Assumptions

- The existing React `src/pages/Sqoba.tsx` remains the behavioral baseline for this migration slice.
- The existing Vue `useSettingsPageState()` composable is the canonical source for editable backup/compression settings and manifest refresh status in Vue.
- Keeping some text in English is acceptable if it matches the current Vue migration surface better than mixing partially mojibaked strings from older files.

## Verification Plan

1. Run TypeScript verification for Arrancador from `apps/arrancador`.
2. Run the Vue renderer build from `apps/arrancador`.
3. Capture raw outputs in `apps/arrancador/.agent/tasks/2026-04-20-arrancador-vue-sqoba-page-worker-c/raw/`.
4. Write `evidence.md` and `evidence.json` with AC-by-AC status based on the current code and fresh command results.
