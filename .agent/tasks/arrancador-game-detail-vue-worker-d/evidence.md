# Evidence

## Scope Verified

- Vue route wiring now points `/game/:id` to `GameDetailPage` in `apps/arrancador/src-vue/router.ts`.
- The Vue implementation lives entirely under `apps/arrancador/src-vue/**`.
- Supporting Vue composable added: `apps/arrancador/src-vue/composables/useGameStatus.ts`.

## Commands

1. `bun run typecheck`
   - Result: `PASS`
   - Raw artifact: `typecheck.txt`

2. `bunx vitest run --configLoader native --config src-vue/test/vitest.config.mjs`
   - Result: `PASS`
   - Raw artifact: `vue-tests.txt`

3. `rg -n 'path: "game/:id"|component: GameDetailPage|import GameDetailPage' apps/arrancador/src-vue/router.ts`
   - Result: `PASS`
   - Raw artifact: `route-check.txt`

4. `rg -n "checkRestoreNeeded|shouldBackupBeforeLaunch|checkBackupNeeded|subscribeAppEvent|backup:progress|restore:progress|toggleBackupEnabled|findGameSavePaths|pickDirectoryPath|pickFilePath|user_rating|user_note|metadataApi\\.search|metadataApi\\.apply|background_image|cover_image" apps/arrancador/src-vue/pages/GameDetailPage.vue`
   - Result: `PASS`
   - Raw artifact: `behavior-check.txt`

## Acceptance Criteria Status

- AC1: `PASS`
  - Route import and route component wiring were confirmed in `route-check.txt`.
- AC2: `PASS`
  - Current page code derives hero image, genres, description, and hero metadata from the current game and renders a description modal.
- AC3: `PASS`
  - Current page code uses `checkRestoreNeeded`, `shouldBackupBeforeLaunch`, `checkBackupNeeded`, install/running status polling, process kill flow, and backup/restore progress event subscriptions.
- AC4: `PASS`
  - Current page persists favorite toggle, play status, user rating, and user note through the Vue store / existing APIs.
- AC5: `PASS`
  - Current page performs metadata search and apply via `metadataApi.search` and `metadataApi.apply`.
- AC6: `PASS`
  - Current page loads backup history, supports manual backup creation, restore actions, and a backup-enabled toggle.
- AC7: `PASS`
  - Current page supports save-path lookup, file/folder pickers, `{PATHTOGAME}` insertion, open-path behavior, and save-path persistence.
- AC8: `PASS`
  - Current page exposes an edit dialog for name, description, background image URL, and cover image URL.
- AC9: `PASS`
  - New Vue files use Composition API `<script setup lang="ts">` and stay inside `src-vue/**`.
- AC10: `PASS`
  - Evidence bundle and verifier verdict files were written under `.agent/tasks/arrancador-game-detail-vue-worker-d/`.

## Notes

- An optional `bun run build:renderer:vue` attempt earlier in the session failed in the local environment with a Tailwind/native binary loading issue (`@tailwindcss/oxide` / `spawn EPERM`). That failure is not specific to the Game Detail changes, and the required verification surface for this task remains `PASS` via typecheck plus Vue tests.

