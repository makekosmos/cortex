# Evidence - Eden settings sidebar shell reuse fix

## Summary
The Eden settings/object-types shell regression was fixed by moving settings and object type navigation back into the shared outer `EdenSidebar` hosted by `DesktopChrome`.

The content panes for settings and object types no longer render their own sidebars, so the app now keeps the same outer sidebar element, width, and hidden state while switching screens.

## Code Evidence
- Shared outer sidebar now switches by mode in `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
  - `notes` mode: create, search, recent, all objects, settings footer
  - `settings` mode: settings nav items and back button
  - `object-types` mode: new type button, system/custom type lists, back button
- Settings content is content-only in `apps/eden/ts/src/components/settings/SettingsPage.vue`
  - no `KosmosSidebar` import
  - no sidebar config persistence inside settings content
- Object types content is editor-only in `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
  - no `ObjectTypesSidebar` import
  - selected type sync emitted back to `App.vue`
  - create-draft token added so outer sidebar can open a new type draft
- App shell wiring in `apps/eden/ts/src/App.vue`
  - passes `active-settings-tab` and `selected-object-type-id` into `EdenSidebar`
  - routes sidebar actions to `openSettingsTab`, `openObjectTypes`, `createObjectType`, and `handleSidebarBack`
  - keeps `ObjectTypesSettings` and `SettingsPage` as content panes only

## Verification

### AC1
PASS

`DesktopChrome` keeps rendering one `EdenSidebar`, and the sidebar is reused across `notes`, `settings`, and `object-types` by switching item sets inside `EdenSidebar.vue`.

### AC2
PASS

`SettingsPage.vue` and `ObjectTypesSettings.vue` no longer import or render their own sidebar components.

Raw artifact:
- `raw/sidebar-shell-check.txt`

### AC3
PASS

Sidebar width/hidden state continue to come from `layout.widgetSidebarWidth` and `layout.widgetSidebarHidden` in `App.vue`, and because the same outer sidebar remains mounted, those values persist across screen changes.

### AC4
PASS

Back navigation is restored through the shared sidebar:
- settings -> back to notes
- object types -> back to settings

This is wired through `handleSidebarBack()` in `App.vue` and `topItem` in `EdenSidebar.vue`.

### AC5
PASS

Object type rows in the shared sidebar use the actual type icon and color:
- `iconSrc: /anytype/icon/type/default/...`
- `iconColor: noteType.color`

This reuses the shared visuals sidebar icon-mask support already added in `packages/kosmos-visuals/components/Sidebar.vue`.

### AC6
PASS

`ObjectTypesSettings.vue` remains usable as an editor-only pane:
- existing types open through `initialSelectedTypeId`
- new type drafts open through `createDraftToken`
- selected type sync flows back to `App.vue`

### AC7
PASS

UTF-8 spot checks for touched Russian strings succeeded.

Raw artifact:
- `raw/utf8-check.txt`

### AC8
PASS

Checks:
- `bun x tsc --noEmit`
- `bun run build`
- `bun run dev:web`

Raw artifacts:
- `raw/tsc.txt`
- `raw/tsc-rerun.txt`
- `raw/build.txt`
- `raw/build-rerun.txt`
- `raw/dev-web.txt`

## Environment Notes
- A Playwright smoke launch was attempted from `apps/eden/ts`, but Chromium launch failed with `spawn EPERM`.
- This is an environment/runtime restriction, not a code failure in the Eden app.
- The attempted output is saved at `raw/playwright-smoke.txt`.
