# Task Spec - Kepler settings sidebar visuals

## Goal

Introduce a shared settings sidebar surface in `@kosmos/visuals` and migrate Kepler settings to it so the settings window gets the new sidebar layout, tokenized dark acrylic background, grouped navigation, and inline search across settings sections/subsections.

## Scope

- `packages/visuals/`
- `shell/src/views/SettingsView.vue`

## Acceptance Criteria

- AC1: `@kosmos/visuals` exports a new shared settings sidebar shell component that uses dedicated theme variables for the main background, border, and settings-specific surfaces instead of hardcoded per-view colors.
- AC2: The shared settings sidebar shell supports a title prop and accepts arbitrary child content so consumers can pass custom controls like search and grouped navigation inside it.
- AC3: Kepler settings uses the new shared sidebar shell with 8px outer padding, a right border, the window title at the top, and three unlabelled sidebar groups arranged with 26px spacing: search, main settings, and advanced settings.
- AC4: The sidebar search field matches the requested visual spec: reusable background token, Lucide search icon, placeholder/foreground/focus states from variables, 4px radius, text cursor, and no hover treatment.
- AC5: Settings navigation rows match the requested shared visual spec: fixed-width rows, 13px medium Inter text, 22x22 icon surface, 6px radius, no hover state, and active state background `#343434`.
- AC6: Search filters settings and subsettings across the available pages; when there are no matches, the sidebar shows `Ничего не найдено` and the content pane does not render unrelated settings rows.
- AC7: The settings window still builds and typechecks after the sidebar migration via the existing shell build pipeline.

## Verification Plan

- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`

## Raw Artifact Targets

- `.agent/tasks/2026-05-23-settings-sidebar-visuals/raw/shell-typecheck.txt`
- `.agent/tasks/2026-05-23-settings-sidebar-visuals/raw/shell-build-js.txt`
