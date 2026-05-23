# Task Spec - Kepler settings advanced layout

## Goal

Move Focus into the advanced settings group and replace the settings content header with layout-specific chrome: basic pages show navigation/window controls, while advanced pages show the same controls plus a disabled gray feature toggle placeholder.

## Scope

- `shell/src/views/SettingsView.vue`

## Acceptance Criteria

- AC1: `Фокус` appears in the advanced settings group instead of the main settings group.
- AC2: Settings content no longer shows the page title text (`Общие`, `Фокус`, etc.) at the top of the content pane.
- AC3: The top content chrome no longer has a bottom border.
- AC4: Basic pages render back, forward, minimize, and close controls; maximize/restore is not shown.
- AC5: Advanced pages render the same controls plus a disabled gray toggle placeholder that cannot be switched yet.
- AC6: Existing page routing, search filtering, and per-page data loading continue to work.
- AC7: Shell typecheck and build pass after the layout change.

## Verification Plan

- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`

## Raw Artifact Targets

- `.agent/tasks/2026-05-23-settings-advanced-layout/raw/shell-typecheck.txt`
- `.agent/tasks/2026-05-23-settings-advanced-layout/raw/shell-build-js.txt`
