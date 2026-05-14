# Evidence — Eden sidebar migration to @kosmos/visuals

## Scope
Relevant files for this sidebar migration pass:
- `packages/kosmos-visuals/components/Sidebar.vue`
- `packages/kosmos-visuals/components/index.ts`
- `packages/kosmos-visuals/index.ts`
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
- `apps/eden/ts/src/components/settings/SettingsPage.vue`
- `apps/eden/ts/src/components/settings/SpacesSettings.vue`
- `apps/eden/ts/src/composables/useTitlebarSafeArea.ts`
- `apps/eden/ts/tests/app.spec.ts`
- `apps/eden/ts/tests/hevy.spec.ts`
- `apps/eden/ts/src/components/sidebar/MainSidebar.vue` (removed)
- `apps/eden/ts/src/components/sidebar/WidgetSidebar.vue` (removed)
- `apps/eden/ts/src/components/sidebar/SpaceRail.vue` (removed)
- `apps/eden/ts/src/components/sidebar/VaultRail.vue` (removed)
- `apps/eden/ts/src/components/sidebar/VaultSidebar.vue` (removed)

## What changed
- Eden main shell now uses a **thin shared sidebar adapter** (`EdenSidebar.vue`) built on the same `Sidebar` contract shape as Delphi: `primaryItems`, `projectItems`, `footerItems`.
- Eden ordinary notes UI now renders **one** shared sidebar instead of separate vault + widget sidebars.
- `SettingsPage.vue` now uses the same shared sidebar contract instead of its own custom left nav.
- Space selection moved into new `SpacesSettings.vue` and is no longer exposed as a dedicated main-shell sidebar surface.
- The shared sidebar API stays aligned with Delphi’s direct usage pattern; the Eden-specific custom sidebar chrome path was removed.
- macOS-style top safe area / traffic-light spacing continues to come from the shared sidebar shell and Eden settings spacing.
- Vault switching is no longer a custom sidebar surface in ordinary UI; the main sidebar now stays visually aligned with Delphi, and vault path switching lives in settings.
- Dead Eden-local sidebar shells removed:
  - `MainSidebar.vue`
  - `WidgetSidebar.vue`
  - `SpaceRail.vue`
  - `VaultRail.vue`
  - `VaultSidebar.vue`

## Verification
### Fresh commands
- `bun run lint` ✅
- `bunx tsc --noEmit -p tsconfig.json` ✅
- `bun run build` ✅
- `bun run test:e2e` ✅ `19 passed`
- Focused Playwright rerun ✅ `3 passed` covering:
  - spaces/settings flow
  - single shared sidebar collapse/expand
  - vault switching available from settings instead of ordinary sidebar

## Import / contract evidence
- Eden settings sidebar imports shared sidebar through public API:
  - `apps/eden/ts/src/components/settings/SettingsPage.vue`
- Eden main shell sidebar adapter imports shared sidebar through public API:
  - `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
- Shared package public contract updated in:
  - `packages/kosmos-visuals/components/Sidebar.vue`
  - `packages/kosmos-visuals/components/index.ts`
  - `packages/kosmos-visuals/index.ts`

## Behavioral evidence
- Main shell still supports:
  - create note
  - search
  - recent note open
  - settings open
- Main notes UI no longer renders `.vault-sidebar-wrapper`; tests assert a single shared sidebar path and no vault-specific custom sidebar chrome.
- Eden main sidebar now follows the same public `KosmosSidebar` usage shape as Delphi (`primaryItems` + `projectItems` + `footerItems`).
- Settings sidebar supports:
  - general
  - trash
  - storage
  - connected apps
  - object types
  - spaces
- Space selection from settings drives:
  - all objects
  - all notes
  - all properties
  - diary
- Vault switching now belongs to Settings / general storage controls instead of a second ordinary-UI sidebar path.
- Slash command popup still works after the migration.

## Remaining risks
- `main/store.ts` and `layout.ts` still carry legacy vault sidebar persistence fields even though the UI now uses one main shared sidebar; this is non-blocking follow-up cleanup.
