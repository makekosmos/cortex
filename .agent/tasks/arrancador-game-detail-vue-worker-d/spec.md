# Task Spec: Arrancador Game Detail Vue Port

## Task Source

Original task statement:

> You are worker D on the Arrancador React->Vue/Vapor migration in `D:\Personal\Hobby\Coding\kosmos\apps\arrancador`. Ownership: port the Game Detail route under `apps/arrancador/src-vue/**` only, including any supporting components/composables in `src-vue/components/game-detail` or `src-vue/composables` as needed. Do not touch React `src/` files. You are not alone in the codebase; do not revert others' edits, and adapt to concurrent changes. Task: port the current React GameDetail page into Vue using Composition API `<script setup lang="ts">`, reusing the existing APIs from `src/lib/api.ts` and browser helpers. Preserve the core behaviors: show game hero/details, launch flow, favorite toggle, play status, metadata search/apply, backups list/restore/create, save-path lookup/edit, user rating/note, and edit dialog for basic fields. Keep the current Tailwind visual language; it is acceptable to simplify some layout details if behavior stays intact. At the end, report changed files and any remaining gaps. Edit files directly in your workspace.

## Scope

Port the current React `GameDetail` page into the Vue app under `apps/arrancador/src-vue/**`, replacing the placeholder `/game/:id` route with a working Vue page and adding only the Vue-side supporting files needed for the feature.

## Constraints

- Do not edit React files under `apps/arrancador/src/**`.
- Do not revert or overwrite unrelated concurrent changes.
- Use Vue 3 Composition API with `<script setup lang="ts">`.
- Reuse APIs from `apps/arrancador/src/lib/api.ts` and browser helpers from `apps/arrancador/src/lib/browser.ts`.
- Preserve the current Tailwind-based visual language, but exact layout parity is not required.
- Keep route/page components as composition surfaces; move feature-specific UI or state into focused Vue components/composables where justified.

## Non-goals

- Do not migrate SQOBA as a separate route.
- Do not add new backend or IPC APIs.
- Do not refactor unrelated Vue pages or shared app shell code beyond what is required to wire the Game Detail route.
- Do not attempt to make the Vue page pixel-identical to the React page.

## Assumptions

- The existing Vue app uses the shared type definitions from `apps/arrancador/src/types`.
- The Vue store in `src-vue/stores/games.ts` is the source of truth for the current game list and should be refreshed after mutations that affect game data.
- Browser-native `window.confirm`/`window.alert` are acceptable for destructive or blocking flows where the React page already relied on them.
- Focused supporting files under `src-vue/components/game-detail` and `src-vue/composables` are allowed when they reduce route complexity.

## Component Map

- `pages/GameDetailPage.vue`
  - Route-level orchestration surface for loading the game by id, wiring actions, and composing the feature sections/modals.
- `components/game-detail/GameDetailHero.vue`
  - Present the hero image, title, summary metadata, favorite state, and primary launch actions.
  - Props: current game state, derived launch/install/running state, busy flags.
  - Emits: launch, back, toggle-favorite, open-description, open-edit.
- `components/game-detail/GameDetailBackupsPanel.vue`
  - Present backup status, list, restore/create actions, and save-path editing/lookup controls.
  - Props: game, backups, loading flags, save path draft, progress, derived display values.
  - Emits: create-backup, restore-backup, lookup-save-path, save-save-path, select-save-folder, select-save-file, open-save-path, insert-game-token, update:save-path-draft, toggle-backup-enabled.
- `components/game-detail/GameDetailMetadataDialog.vue`
  - Present metadata search/apply UI with rename toggle and result selection.
  - Props: open state, query, results, busy flags, rename toggle.
  - Emits: close, search, apply, update:query, update:rename.
- `components/game-detail/GameDetailEditDialog.vue`
  - Present basic editable fields for the game and image search shortcuts.
  - Props: open state, form model, saving flag.
  - Emits: close, save, search-image, update:form.
- `composables/useGameStatus.ts`
  - Vue port of install/running-state polling logic for a single game.

The component map may be collapsed slightly during implementation if a child component would be trivial, but the route page must remain a thin composition surface rather than a single large monolith.

## Acceptance Criteria

- AC1: The Vue router no longer points `/game/:id` at a placeholder; it renders a Vue Game Detail page that resolves the current game from the Vue games store by route param and handles missing ids safely.
- AC2: The page shows the game hero/details using the current game data, including image fallback behavior, summary metadata, and a full-description affordance when a description exists.
- AC3: The page preserves the launch flow using existing APIs/helpers, including install/running detection, launch action, process-kill confirmation when already running, backup-before-launch prompt, restore-before-launch prompt, and progress event subscription display.
- AC4: The page supports favorite toggling, play status updates, user rating updates, and user note editing, with mutations persisted through existing game APIs and reflected in the Vue store after refresh/update.
- AC5: The page supports metadata search/apply using existing RAWG APIs, including query entry, result listing, rename-from-metadata toggle, and applying a selected result to the current game.
- AC6: The page supports backup management for the current game using existing backup APIs, including backup list loading, manual backup creation, restore action, backup-enabled toggle, and reasonable empty/loading states.
- AC7: The page supports save-path lookup and editing using existing backup/browser helpers, including locate-save-path, folder/file selection, `{PATHTOGAME}` insertion, open-path behavior, and persistence of the edited save path.
- AC8: The page supports editing the game’s basic fields in a dialog, at minimum name, description, background image URL, and cover image URL, with save and close flows.
- AC9: All new Vue files use Composition API with `<script setup lang="ts">`, explicit typed props/emits where applicable, and keep feature logic/components inside `src-vue/**` only.
- AC10: Verification artifacts are created under `.agent/tasks/arrancador-game-detail-vue-worker-d/`, including `evidence.md`, `evidence.json`, and a fresh verifier judgment based on the current codebase and current command results.

## Verification Plan

1. Run targeted search/inspection to confirm the route now points to the Vue Game Detail page and supporting files stay inside `src-vue/**`.
2. Run targeted tests or type checks for the Vue app surface if available and practical.
3. Run a fresh verification pass against the current repository state and record outcomes in `.agent/tasks/arrancador-game-detail-vue-worker-d/evidence.md`, `.agent/tasks/arrancador-game-detail-vue-worker-d/evidence.json`, and `.agent/tasks/arrancador-game-detail-vue-worker-d/verdict.json`.
4. If any acceptance criterion is not proven as `PASS`, write `.agent/tasks/arrancador-game-detail-vue-worker-d/problems.md`, apply the smallest safe fix, and reverify.
