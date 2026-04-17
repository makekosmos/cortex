# Task Spec: Restore ResizableSidebar behavior in `@kepler/visuals` `Sidebar`

## Original Task

Изучи историю гита, раньше была приколюха `ResizableSidebar`, вот я хочу ввести его функционал в сайдбар сейчас `packages/kepler-visuals` sidebar.

## Scope

Restore the `ResizableSidebar` behavior that was lost when resize logic moved into [`packages/kepler-visuals/components/Sidebar.vue`](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/Sidebar.vue), while keeping the current structured `Sidebar` API.

## Assumptions

- The relevant lost behavior is the old `ResizableSidebar` feature set that previously backed `Sidebar`: built-in sidebar wrapper/resize styles and optional drag-region support.
- The task does not require reviving `ResizableSidebar.vue` as a public component unless parity cannot be achieved inside `Sidebar.vue`.

## Constraints

- Make the smallest safe diff in `packages/kepler-visuals`.
- Preserve the current `Sidebar` props/events and existing consumers in `apps/dashboard`, `apps/delphi`, and `apps/eden`.
- Follow Vue 3 Composition API patterns already used in the package.

## Non-goals

- Redesigning the visual appearance of the sidebar.
- Refactoring sidebar consumers.
- Reintroducing the removed slot-based `ResizableSidebar.vue` component.

## Acceptance Criteria

- AC1: `Sidebar.vue` includes the base resizable-sidebar wrapper/handle styling required for its resize/collapse behavior without requiring each consumer to import `components/sidebar.css` manually.
- AC2: `Sidebar.vue` restores the old optional `dragRegion` behavior from `ResizableSidebar`, so consumers can opt into `-webkit-app-region: drag` on the sidebar surface while interactive descendants remain clickable.
- AC3: Existing `Sidebar` usage remains compatible, and the dashboard sidebar resize flow still verifies successfully against the current codebase.

## Component Map

- `Sidebar.vue`: owns sidebar state, resize/toggle behavior, and the public API for layout-related behavior.
- `SidebarButton.vue`: presentational nav button/link component; no behavior changes expected for this task.

## Verification Plan

1. Inspect git history for the removed `ResizableSidebar.vue` contract and compare it with current `Sidebar.vue`.
2. Run focused formatting/checks for `@kepler/visuals`.
3. Run the dashboard sidebar verification that exercises the resize handle against the current app.
