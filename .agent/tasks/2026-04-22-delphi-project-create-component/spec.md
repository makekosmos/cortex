# Delphi Project Create Component

## Original Task Statement

User request summary:

- create a convenient component for creating projects in Delphi.

## Summary

Add a focused project-creation UX to Delphi so users can create a project without leaving the current flow or manually editing data elsewhere.

The project creation entry point should live next to the existing project list in the sidebar, open a small dedicated creation UI, and create a normal Delphi project through the current store/local-db/sync pipeline.

## Component Map

- `ProjectCreatePopover.vue`
  - Responsibility: collect project creation fields and emit a typed save/cancel contract.
  - Props: `open`, `existingTitles?`
  - Emits: `update:open`, `save`
- `SideBar.vue`
  - Responsibility: own the open/close state for the creator, invoke store creation, and navigate to the new project after save.
  - Passes project groups and the sidebar header action into shared visuals.
- `packages/kosmos-visuals/components/Sidebar.vue`
  - Responsibility: expose an optional per-group header action button without owning Delphi business logic.

## Acceptance Criteria

- AC1: Delphi sidebar exposes an obvious project creation affordance directly in the projects section, without adding a second unrelated screen.
- AC2: Activating that affordance opens a dedicated project creation UI with at least:
  - title
  - optional notes
  - optional color tag
- AC3: Saving with a non-empty title creates a normal Delphi project via `store.addProject(...)`, persists through the existing project pipeline, and closes the creation UI.
- AC4: After successful creation, Delphi navigates to the created project's route so the user lands inside the project immediately.
- AC5: The component prevents accidental empty-project creation.
- AC6: Existing sidebar project navigation remains intact; this task must not break project listing, active-state highlighting, or settings navigation.
- AC7: Shared `kosmos-visuals` sidebar changes remain additive and backward-compatible for Eden/dashboard and any other existing consumers.
- AC8: TypeScript for Delphi remains clean after the change.

## Constraints

- Keep the UI compact and aligned with the current Delphi visual language.
- Prefer explicit props/emits over hidden mutable shared state.
- Reuse the current project model/store contract instead of inventing a second creation path.
- Avoid broad route/page refactors; this is a focused UX improvement.

## Non-Goals

- Full project editing workflow redesign.
- Area management redesign.
- Nested projects / task hierarchy.
- Ark object-level project persistence changes beyond the existing Delphi store behavior.

## Assumptions

- A “convenient component” means an inline or popover-style creator that is reachable from the sidebar project section.
- `title`, `notes`, and `colorTag` are the relevant first-step project fields because those are already supported by `CreateProjectParams`.
- Navigating into the new project immediately is the least surprising post-create behavior.

## Verification Plan

1. Typecheck Delphi TypeScript.
2. Inspect the final sidebar integration diff to confirm the entry point lives in the projects section.
3. Verify the creation component uses typed props/emits and calls `store.addProject(...)`.
4. Record results in `.agent/tasks/2026-04-22-delphi-project-create-component/`.
