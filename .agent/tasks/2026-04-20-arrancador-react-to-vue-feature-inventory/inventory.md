# Arrancador React -> Vue/Vapor Migration Inventory

## 1. Local Vue/Vapor references already present in this repo

These are useful migration references and confirm that the repo already contains working Vue stacks:

- `apps/dashboard`
  - Vue `3.6.0-beta.9`
  - `@vue/runtime-vapor`
  - `@vitejs/plugin-vue`
  - `vue-router`
  - `vaporInteropPlugin`
  - Entry/reference files:
    - [package.json](/D:/Personal/Hobby/Coding/kepler/apps/dashboard/package.json)
    - [vite.config.mjs](/D:/Personal/Hobby/Coding/kepler/apps/dashboard/vite.config.mjs)
    - [src/main.ts](/D:/Personal/Hobby/Coding/kepler/apps/dashboard/src/main.ts)

- `apps/eden/ts`
  - Vue `3.6.0-beta.9`
  - `@vitejs/plugin-vue` with `vaporInterop: true`
  - Pinia
  - Entry/reference files:
    - [package.json](/D:/Personal/Hobby/Coding/kepler/apps/eden/ts/package.json)
    - [vite.config.mjs](/D:/Personal/Hobby/Coding/kepler/apps/eden/ts/vite.config.mjs)
    - [src/main.ts](/D:/Personal/Hobby/Coding/kepler/apps/eden/ts/src/main.ts)

- `apps/delphi/ts`
  - Stable Vue `3.5.31`
  - Vue Router `5`
  - Pinia
  - Vitest
  - `reka-ui`
  - Entry/reference files:
    - [package.json](/D:/Personal/Hobby/Coding/kepler/apps/delphi/ts/package.json)
    - [vite.config.mjs](/D:/Personal/Hobby/Coding/kepler/apps/delphi/ts/vite.config.mjs)
    - [src/main.ts](/D:/Personal/Hobby/Coding/kepler/apps/delphi/ts/src/main.ts)

Conclusion:
- The repo already has usable local patterns for both stable Vue and Vue + vapor interop.
- Arrancador migration can and should reuse these local precedents instead of inventing a fresh stack.

## 2. Current Arrancador frontend scope

Renderer app:
- [apps/arrancador/package.json](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/package.json)
- [src/main.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/main.tsx)
- [src/router.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/router.tsx)
- [src/providers.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/providers.tsx)
- [vite.config.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/vite.config.ts)

Current renderer size indicators:
- 10 route-level page components in `src/pages/`
- 10 shared components in `src/components/`
- 12 UI primitive wrapper components in `src/components/ui/`
- 4 custom hooks in `src/hooks/`
- 1 React context store in `src/store/`
- 30+ renderer test suites in `src/test/`

## 3. Product features to preserve during migration

This section is the feature contract. These are not “React features”; these are product behaviors that must survive the move to Vue.

### 3.1 App shell and navigation

Files:
- [src/pages/Layout.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Layout.tsx)
- [src/components/Sidebar.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/Sidebar.tsx)
- [src/components/AppTitlebar.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/AppTitlebar.tsx)
- [src/components/TitlebarHistoryControls.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/TitlebarHistoryControls.tsx)
- [src/components/Spotlight.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/Spotlight.tsx)

Feature list:
- Desktop shell with custom titlebar
- Native/fallback window controls
- Back/forward history controls
- Responsive layout with desktop sidebar + mobile sheet menu
- Persistent sidebar width and hidden state
- Sidebar keyboard toggle shortcut
- Quick game search / spotlight overlay with keyboard shortcut
- Route outlet composition and route fallback skeletons

### 3.2 Library page

File:
- [src/pages/Library.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Library.tsx)

Feature list:
- Main game library listing
- Grid/list display modes
- Persisted library filter preset
- Sort modes:
  - name
  - last played
  - date added
  - play count
  - playtime
- Advanced filters:
  - favorites
  - genres
  - platforms
  - played/unplayed
  - installed/not installed
  - play status
  - user rating / metacritic ranges
  - playtime ranges
  - metadata required
- Drag-and-drop add from `.exe` / `.lnk`
- Duplicate detection and shortcut resolution
- RAWG metadata queue prompt after adding games
- Empty states and loading states

### 3.3 Game detail page

File:
- [src/pages/GameDetail.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/GameDetail.tsx)

Feature list:
- Game hero with localized genres and metadata
- Launch button with running/missing/ready states
- Running-instance tracking
- Install-status detection
- Favorite toggle
- Delete game flow
- Edit game metadata dialog
- Metadata search against RAWG
- Apply RAWG metadata to existing game
- Rename from metadata option
- Backup management:
  - load backups
  - create backup
  - restore backup
  - delete backup
  - backup-needed / restore-needed checks
  - backup progress event subscription
- Save-path editing and lookup
- Play status editing
- User rating modal
- User note editing
- Full description modal
- Open external/store/system paths
- Game settings subviews and prompt dialogs

### 3.4 Scan page

File:
- [src/pages/Scan.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Scan.tsx)

Feature list:
- Folder scan tab
- Running processes tab
- Streamed executable scan via IPC
- Scan cancel support
- Per-result selection and rename before add
- Process usage polling
- Existing-game detection
- Add selected scan/process results to library
- RAWG metadata queue after add
- Drag-and-drop of executables/shortcuts

### 3.5 Catalogue page

File:
- [src/pages/Catalogue.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Catalogue.tsx)

Feature list:
- RAWG showcase / search page
- Search input + async search state
- Add RAWG item to local library
- Duplicate detection by RAWG id and normalized game name
- Rollback if metadata apply fails
- Localized genre display in catalogue cards
- Link-through to already-added game detail

### 3.6 Statistics page

File:
- [src/pages/Statistics.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Statistics.tsx)

Feature list:
- Playtime stats fetch by date range
- Preset ranges
- Custom date ranges
- Month-range helpers
- Summary stats
- Charts:
  - area chart
  - bar chart
- Formatted date/time display and tooltips

### 3.7 Settings page

File:
- [src/pages/Settings.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Settings.tsx)

Feature list:
- Appearance/theme settings
- System settings
- Backup settings
- SQOBA settings entry points
- RAWG API key settings
- Start minimized / autostart support
- Manifest refresh action
- Section chips with scroll-to-section behavior

### 3.8 Achievements page

File:
- [src/pages/Achievements.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Achievements.tsx)

Feature list:
- Load achievements
- Filter all/unlocked/locked
- Seed default achievements
- Record achievement events with trigger/context
- Export achievements to clipboard JSON
- Import achievements from JSON payload
- Unlock progress view / summary cards

### 3.9 System info page

File:
- [src/pages/SystemInfo.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/SystemInfo.tsx)

Feature list:
- Load/cached system information
- Display CPU, memory, GPU, storage and uptime data
- Disk speed tests per mount point
- Cache snapshot in localStorage

### 3.10 SQOBA page

File:
- [src/pages/Sqoba.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Sqoba.tsx)

Feature list:
- SQOBA overview cards
- Save-path lookup across library
- Save-file lookup per game
- Filter missing save paths
- Edit/save save-path templates
- Path resolution with `{PATHTOGAME}`
- Open path actions
- Backup settings integration
- Manifest refresh integration

## 4. Shared business components/features to preserve

### 4.1 Shared components

Files:
- [src/components/GameCard.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/GameCard.tsx)
- [src/components/RawgMetadataPrompt.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/RawgMetadataPrompt.tsx)
- [src/components/Spotlight.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/Spotlight.tsx)
- [src/components/Sidebar.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/Sidebar.tsx)
- [src/components/AppTitlebar.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/AppTitlebar.tsx)
- [src/components/mode-toggle.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/mode-toggle.tsx)

These are feature-bearing and must be ported, not just visually replicated:
- Game card link behavior
- Metadata application prompt flow
- Spotlight query, ranking and keyboard control
- Sidebar navigation state + resize/collapse
- Titlebar integration with window chrome
- Theme toggle behavior

### 4.2 Global providers / global state

Files:
- [src/providers.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/providers.tsx)
- [src/store/GamesContext.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/store/GamesContext.tsx)
- [src/components/theme-provider.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/theme-provider.tsx)
- [src/components/language-provider.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/language-provider.tsx)
- [src/components/ToastProvider.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ToastProvider.tsx)

Migration targets:
- `GamesContext` -> Pinia store or provide/inject composable store
- `ThemeProvider` -> Vue composable/provider
- `LanguageProvider` -> Vue composable/provider or i18n-lite store
- `ToastProvider` -> Vue toast service/store + portal/teleport rendering
- Provider composition -> Vue app boot + plugins

## 5. React-specific implementation layers that must be replaced

This is the direct rewrite surface.

### 5.1 Entry and routing

Current React files:
- [src/main.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/main.tsx)
- [src/router.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/router.tsx)
- [src/App.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/App.tsx)

Current React-specific behavior:
- `ReactDOM.createRoot`
- `RouterProvider`
- `createBrowserRouter`
- `lazy()` + `Suspense`
- `Outlet`, `useNavigate`, `useLocation`, `useNavigationType`, `Link`, `NavLink`

Vue migration target:
- `createApp`
- `createRouter`
- route records with lazy imports
- Vue Router navigation composables
- `<RouterView>`
- optionally vapor interop plugin if targeting Vapor-compatible path

### 5.2 State and hooks model

Current React-specific patterns:
- `useState`
- `useEffect`
- `useMemo`
- `useCallback`
- `useRef`
- React contexts
- `forwardRef`

Affected files include:
- [src/store/GamesContext.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/store/GamesContext.tsx)
- [src/hooks/useSettingsState.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/hooks/useSettingsState.ts)
- [src/hooks/useGameStatus.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/hooks/useGameStatus.ts)
- [src/hooks/useDropZone.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/hooks/useDropZone.ts)
- [src/hooks/use-mobile.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/hooks/use-mobile.ts)

Vue migration target:
- composables
- `ref`, `reactive`, `computed`, `watch`
- Pinia or injected stores
- template refs / `useTemplateRef` where needed

### 5.3 UI primitive layer

Current React UI wrappers:
- [button.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/button.tsx)
- [card.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/card.tsx)
- [dropdown-menu.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/dropdown-menu.tsx)
- [input.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/input.tsx)
- [progress.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/progress.tsx)
- [scroll-area.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/scroll-area.tsx)
- [separator.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/separator.tsx)
- [sheet.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/sheet.tsx)
- [sidebar.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/sidebar.tsx)
- [skeleton.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/skeleton.tsx)
- [switch.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/switch.tsx)
- [tooltip.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/components/ui/tooltip.tsx)

Current React-specific vendor dependencies:
- `@radix-ui/react-dialog`
- `@radix-ui/react-dropdown-menu`
- `@radix-ui/react-progress`
- `@radix-ui/react-scroll-area`
- `@radix-ui/react-separator`
- `@radix-ui/react-slot`
- `@radix-ui/react-tooltip`

Vue migration target:
- port wrappers to Vue SFCs
- likely use `reka-ui` patterns already present in `delphi`
- or replace with app-local Vue primitives where simpler

### 5.4 Icon layer

Current:
- `lucide-react`

Migration target:
- `lucide-vue-next`

### 5.5 Charts layer

Current:
- `recharts`
- used in [src/pages/Statistics.tsx](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/pages/Statistics.tsx)

Migration target:
- replace charting with a Vue-capable chart library or local wrapper
- this is a product feature dependency, not just syntax conversion

## 6. Framework-agnostic backend/frontend contract that should stay intact

These are not React-specific and should be preserved as-is or minimally adapted:

- [src/lib/api.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/lib/api.ts)
- [src/lib/browser.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/lib/browser.ts)
- [src/lib/ipc.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/lib/ipc.ts)
- [src/types/index.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/types/index.ts)

API groups already defined:
- `gamesApi`
- `metadataApi`
- `achievementsApi`
- `backupApi`
- `settingsApi`
- `statsApi`
- `scanApi`
- `systemApi`
- `notificationsApi`
- `catalogueApi`
- `windowApi`

Migration note:
- UI framework can change without rewriting the Electron/main-process contracts.
- The safest migration is to keep these API contracts stable while swapping only the renderer implementation.

## 7. Build, dependency, and test layers that must be migrated

### 7.1 Build/tooling replacements

Current React-specific config:
- [vite.config.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/vite.config.ts)

Current React-specific items:
- `@vitejs/plugin-react`
- React alias resolution
- React dedupe
- `tsx` include/coverage assumptions

Vue migration target:
- `@vitejs/plugin-vue`
- Vue aliases/dedupe
- `.vue` + `.ts` coverage/test config
- optional `vaporInterop: true` based on local dashboard/eden pattern

### 7.2 Dependency replacements

Current React deps in [package.json](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/package.json):
- `react`
- `react-dom`
- `react-router-dom`
- `lucide-react`
- `@radix-ui/react-*`
- `@testing-library/react`
- `@types/react`
- `@types/react-dom`
- `@types/react-router-dom`
- `@vitejs/plugin-react`

Expected Vue-side replacements:
- `vue`
- `vue-router`
- `lucide-vue-next`
- `@vitejs/plugin-vue`
- optionally `pinia`
- optionally `@vue/runtime-vapor` and `vaporInteropPlugin`
- Vue-side test libs as needed alongside `vitest`

### 7.3 Test suites to migrate

Current renderer tests in `src/test/` include:
- `achievements.test.tsx`
- `app-providers.test.tsx`
- `app-titlebar.test.tsx`
- `app.test.tsx`
- `catalogue.test.tsx`
- `game-card.test.tsx`
- `game-detail.test.tsx`
- `games-context.test.tsx`
- `layout.test.tsx`
- `library.test.tsx`
- `main.test.tsx`
- `mode-toggle.test.tsx`
- `rawg-metadata-prompt.test.tsx`
- `router.test.ts`
- `scan.test.tsx`
- `settings.test.tsx`
- `sidebar-component.test.tsx`
- `smoke.test.tsx`
- `spotlight.test.tsx`
- `statistics.test.tsx`
- `system-info.test.tsx`
- `theme-provider.test.tsx`
- `toast-provider.test.tsx`
- `ui-primitives.test.tsx`
- `ui-sidebar.test.tsx`
- `use-game-status.test.tsx`
- `use-mobile.test.tsx`
- `use-settings-state.test.tsx`
- plus non-UI utility/API tests

Migration implication:
- every `.tsx` component/hook/provider test must be ported to Vue test patterns
- router mocks and component mount helpers must be rewritten
- primitive wrapper tests must be recreated against the new Vue primitive layer

## 8. Recommended migration workstreams

This is the concrete “what we need to work through” list.

### Workstream A: Boot and infrastructure

- Replace React entry with Vue app bootstrap
- Replace React Router with Vue Router
- Replace provider composition with Vue plugin/composable setup
- Update Vite config from React to Vue/Vapor

### Workstream B: Global state

- Port `GamesContext` to Pinia or an injected store
- Port theme state/provider
- Port language state/provider
- Port toast service/provider

### Workstream C: Shared shell

- Port `Layout`
- Port `Sidebar`
- Port `AppTitlebar`
- Port `Spotlight`
- Port `TitlebarHistoryControls`

### Workstream D: UI primitives

- Rebuild all `src/components/ui/*` wrappers in Vue
- Replace Radix React usage with Vue primitives/reka-ui/local wrappers

### Workstream E: Feature pages

- Port `Library`
- Port `GameDetail`
- Port `Scan`
- Port `Catalogue`
- Port `Statistics`
- Port `Settings`
- Port `Achievements`
- Port `SystemInfo`
- Port `Sqoba`

### Workstream F: Hooks to composables

- `useSettingsState`
- `useGameStatus`
- `useDropZone`
- `use-mobile`

### Workstream G: Test migration

- Replace React component tests with Vue component tests
- Rebuild router/store test harnesses
- Recreate primitive wrapper test coverage

## 9. Practical migration note

If the goal is “full port, no partial hybrid,” the migration unit is the renderer, not isolated JSX files.

That means:
- do not think of this as “convert components one by one” only
- think of it as:
  - boot/runtime swap
  - state/provider swap
  - primitive layer swap
  - route/page swap
  - test harness swap

The business-facing contracts that must remain stable are:
- IPC/API contracts in `src/lib/api.ts`
- browser/electron helpers in `src/lib/browser.ts`
- domain types in `src/types/index.ts`
- all user-facing feature behavior listed above

## 10. Summary

What exists now:
- full Electron renderer app in React
- 10 major route-level product surfaces
- shared shell, search, toast, theme, language, game store and primitive layers
- React-only vendor stack and React-only tests

What must be ported to Vue:
- all route pages
- all shared shell components
- all providers/stores/hooks
- all UI primitives
- all React router/state/test infrastructure
- all React-specific dependencies

What does not need conceptual redesign:
- Electron/main-process IPC surface
- domain types
- core business capabilities

This is the full migration surface for “перенести всё на Vue” in Arrancador.
