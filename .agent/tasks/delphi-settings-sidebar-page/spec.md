# Delphi Settings Sidebar Page

## Goal

Turn Delphi settings into a dedicated settings shell with its own left sidebar navigation, similar to the settings layout used in `eden/ts`.

## Component Map

- `apps/delphi/ts/src/pages/SettingsPage.vue`
  Route-level composition surface. Owns the active settings tab state from the route query and composes sidebar + tab content.
- `apps/delphi/ts/src/components/settings/GeneralSettingsTab.vue`
  Presents general Delphi settings such as theme selection.
- `apps/delphi/ts/src/components/settings/SpacesSettingsTab.vue`
  Presents sync space management actions: list, rename, delete, and active space indicator.

## Acceptance Criteria

- AC1: Visiting `/settings` shows a two-column settings layout with a dedicated left settings sidebar and a right content area.
- AC2: The left settings sidebar includes at least `Общие` and `Пространства`, and switching items changes the content without leaving `/settings`.
- AC3: The `Общие` section contains the existing theme controls.
- AC4: The `Пространства` section contains the existing space management UI, including rename and delete flows.
- AC5: The selected settings section is reflected in the route query so refresh/back keeps the active section stable.
- AC6: `bunx tsc --noEmit` passes in `apps/delphi/ts`.
