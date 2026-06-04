# Delphi migration to @kosmos/visuals

## Classification

`FULL_LOOP` — application-wide Delphi UI migration touching many Vue components, shared design-system usage, and visual verification.

## Goal

Migrate the desktop Delphi Vue extension UI to the Kosmos design system by replacing local Tailwind-heavy and hardcoded styling with `@kosmos/visuals` components, tokens, and layout primitives where they fit the existing UX.

## Scope

In scope:

- `extensions/delphi/src/**` desktop renderer UI.
- Existing Delphi flows: sidebar navigation, task pages, project page, quick entry/search/open, project creation, settings, and overlays.
- Reuse public exports from `@kosmos/visuals` and runtime CSS tokens from `@kosmos/visuals/theme/css`.
- Keep all user-facing UI copy in Russian.

Out of scope:

- Android/mobile Delphi.
- ARK data model, sync protocol, write-boundary, schema, and `electron-api-shim` behavior.
- Version bumps, release/publish scripts, dependency upgrades, or unrelated refactors.
- Rebuilding `@kosmos/visuals` itself except for tiny compatibility fixes that are necessary for Delphi and safe for existing consumers.

## Acceptance Criteria

**AC1.** Delphi imports the Kosmos visuals theme and uses public `@kosmos/visuals` exports for the app shell/layout and common controls where matching components already exist.

**AC2.** Delphi renderer UI has no hardcoded `#hex`, `rgb(...)`, or `rgba(...)` colors, no custom font-face or custom font-family declarations, and no dominant local Tailwind color utilities for core surfaces where visuals tokens/components cover the same role.

**AC3.** Existing Delphi workflows remain available: open Inbox/Today/task pages, create a task via QuickEntry, open QuickSearch/QuickOpen, create/edit a project, and open settings without runtime crashes.

**AC4.** UI rules are preserved: Russian-only user-facing text, no custom titlebar/safe-area, no nested interactive controls, and no `addEventListener` without matching cleanup.

**AC5.** Verification passes with relevant static checks plus visual verification evidence under `.tmp/` for at least the main Delphi screen, QuickEntry/project dialog, and Settings.

**AC6.** No unrelated files are changed, no version bump/release metadata is changed, and the existing untracked `ARCHITECTURE-PLANS.md` remains untouched.
