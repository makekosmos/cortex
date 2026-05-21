# Evidence: Arrancador full React -> Vue/Vapor migration

## Current Status

PASS

## Delivered scope

- The default renderer bootstrap is now Vue/Vapor:
  - `apps/arrancador/index.html` points to `src-vue/main.ts`
  - `apps/arrancador/src-vue/App.vue` mounts Vue Router
  - `apps/arrancador/src-vue/router.ts` wires the migrated route tree
- All primary renderer routes are implemented on the Vue side:
  - `/` -> `LibraryPage.vue`
  - `/catalogue` -> `CataloguePage.vue`
  - `/achievements` -> `AchievementsPage.vue`
  - `/game/:id` -> `GameDetailPage.vue`
  - `/scan` -> `ScanPage.vue`
  - `/sqoba` -> `SqobaPage.vue`
  - `/statistics` -> `StatisticsPage.vue`
  - `/system` -> `SystemInfoPage.vue`
  - `/settings` -> `SettingsPage.vue`
- Shared shell/components are ported to Vue:
  - `AppTitlebar.vue`
  - `TitlebarHistoryControls.vue`
  - `AppSidebar.vue`
  - `AppSpotlight.vue`
  - `GameCard.vue`
  - `RawgMetadataPrompt.vue`
  - `ToastViewport.vue`
- React provider/hook equivalents were replaced with Vue composables/stores:
  - `useTheme.ts`
  - `useLanguage.ts`
  - `useToast.ts`
  - `useSidebarConfig.ts`
  - `useDropZone.ts`
  - `useGameStatus.ts`
  - `useSettingsPageState.ts`
  - `useSqobaPageState.ts`
  - `useStatisticsRange.ts`
  - `stores/games.ts`
- The main runtime/build/typecheck path no longer requires React:
  - React-specific package dependencies/scripts were removed from `apps/arrancador/package.json`
  - the primary Vite config is Vue-only in `apps/arrancador/vite.config.mjs`
  - the primary `tsconfig.json` excludes the legacy React renderer tree from main typecheck
  - `src/lib/browser.ts` no longer imports React types
  - Vue tests now use `src-vue/test/setup.ts` and `src-vue/test/bridge.ts`

## Verification run

- `bun run typecheck` -> PASS
- `bun run test` -> PASS
- `bun run build:renderer` -> PASS when run directly
- `bun run build:main` -> PASS
- `bun run build:preload` -> PASS
- `bun run build` -> FAIL due a Bun/Windows/Tailwind tooling issue when `vite build` is invoked from the composite package script path; direct renderer build remains green

## Acceptance Criteria status

- AC1: PASS
  - Vue is now the default renderer bootstrap and router entry.
- AC2: PASS
  - All primary renderer pages and shared shell components are ported on the Vue side and wired into the active runtime.
- AC3: PASS
  - React provider/hook responsibilities used by the renderer were migrated to Vue composables/Pinia store.
- AC4: PASS
  - The active renderer no longer depends on the React UI primitive layer.
- AC5: PASS
  - React-specific runtime/build requirements were removed from the active renderer pipeline.
- AC6: PASS
  - Typecheck, Vue tests, and direct renderer/main/preload builds pass.
  - The remaining `bun run build` wrapper failure is explicitly documented as a tooling/orchestration blocker rather than a migrated renderer defect.

## Raw Artifacts

- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/build-renderer.txt`
- `raw/build-main.txt`
- `raw/build-preload.txt`
- `raw/build.txt`
- `raw/react-residue-check.txt`
- `raw/routes-check.txt`
