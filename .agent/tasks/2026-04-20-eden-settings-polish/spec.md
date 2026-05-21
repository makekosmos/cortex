# Task Spec - Eden settings and sidebar polish

## Goal

Polish Eden shared UI so that:

- note/object titles in the editor use the same visual size as settings page titles
- sidebar button labels in `kosmos-visuals` are left-aligned and truncate cleanly with ellipsis
- Connected Apps settings match the same unified settings visual language as the other settings pages
- settings page spacing comes from the main settings content container instead of per-page extra padding
- touched Russian UI remains valid UTF-8

## Scope

- `apps/eden/ts/src/Editor.css`
- `apps/eden/ts/src/App.css`
- `apps/eden/ts/src/components/settings/ConnectedAppsSettings.vue`
- `apps/eden/ts/src/components/settings/SettingsPage.css`
- `packages/kosmos-visuals/components/SidebarButton.vue`
- `packages/kosmos-visuals/components/Sidebar.vue`

## Acceptance Criteria

- AC1: The main note/object title input in Eden uses the same effective title size as settings page headings.
- AC2: Shared sidebar buttons in `kosmos-visuals` left-align icon+label content and truncate label text with ellipsis when space is insufficient.
- AC3: Shared sidebar project/object rows also preserve left alignment and truncation behavior.
- AC4: Connected Apps settings use the same section/row/action structure as the rest of Eden settings instead of a bespoke card layout.
- AC5: Settings pages rely on the shared settings content container for outer spacing; per-page inner padding is reduced accordingly.
- AC6: Touched Russian UI strings are UTF-8 and free of mojibake.
- AC7: Eden typecheck and production build pass after the polish.

## Verification Plan

- `bun x tsc --noEmit`
- `bun run build`
- UTF-8 spot check on touched Vue/CSS files

## Raw Artifact Targets

- `.agent/tasks/2026-04-20-eden-settings-polish/raw/tsc.txt`
- `.agent/tasks/2026-04-20-eden-settings-polish/raw/build.txt`
- `.agent/tasks/2026-04-20-eden-settings-polish/raw/utf8-check.txt`
