# Task Spec - Kepler file search settings page

## Goal

Move file search configuration out of the general settings page into its own advanced settings page named `Поиск файлов`, reachable from the settings sidebar and searchable through settings search.

## Scope

- `shell/src/views/SettingsView.vue`

## Acceptance Criteria

- AC1: Kepler settings sidebar includes a new advanced navigation entry `Поиск файлов`.
- AC2: The existing file search toggle and its supporting copy/error state are removed from the general settings page and rendered on the dedicated `Поиск файлов` page instead.
- AC3: Settings search can find and open the new `Поиск файлов` page through file-search-related keywords.
- AC4: The dedicated `Поиск файлов` page continues to use the existing file search settings state and RPC flow (`file_index.settings_get` / `file_index.settings_set`) without regression.
- AC5: Shell typecheck and build still pass after the settings page split.

## Verification Plan

- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`

## Raw Artifact Targets

- `.agent/tasks/2026-05-23-settings-file-search-page/raw/shell-typecheck.txt`
- `.agent/tasks/2026-05-23-settings-file-search-page/raw/shell-build-js.txt`
