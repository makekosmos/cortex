# Task Spec — Eden sidebar fixed header / bottom settings layout

## Task ID / Path

- `2026-06-14-eden-sidebar-fixed-header-bottom-settings`
- `.agent/tasks/2026-06-14-eden-sidebar-fixed-header-bottom-settings/spec.md`

## Original task statement

> Task: Create a new FULL_LOOP spec for this Eden sidebar follow-up layout task. Do not implement. Keep it concise.
>
> User request summary: sidebar has vertical-height problems: when window height is reduced, sidebar content gets squeezed/overlaps. Settings should be a separate fixed bottom block, similar to Zen mode bottom surface: backing surface and top border/shadow when content scrolls underneath. Only main sidebar content should scroll; header/top actions and settings block should not scroll. Remove the "Eden" title. In the top/header area place four icon buttons: settings, search, new note, sidebar show/hide. When sidebar is collapsed, only sidebar toggle remains visible; settings/search/new note hidden. Add hover background to these buttons using Kosmos visuals token/color. Content should not squeeze/overlap at small vertical heights; it should scroll. Replace padding approach with gutter to avoid asymmetric padding when scrollbar appears.
>
> Scope: Eden sidebar UI/layout only. No data/ARK/schema/sync/write-boundary changes. Acceptance criteria must cover fixed header, fixed bottom settings block, scrollable content only, collapsed visibility behavior, token-based hover, gutter/scrollbar behavior, no overlap at small heights, visual verification required.

## Context

Relevant evidence for freezing the spec:

- `products/eden/src/components/sidebar/EdenSidebar.vue` currently renders a single scroll container with header actions, content groups, and footer items mixed together; it also still shows the `Eden` title in some states.
- `products/eden/src/App.vue` passes `hidden`, search state, recent entries, note types, active screen, and sidebar handlers into `EdenSidebar`.
- `docs-site/apps/eden/ui.md` confirms sidebar/layout changes belong in Eden UI scope and should use `@kosmos/visuals` / CSS vars.
- `docs-site/agents/forbidden/ui.md` bans hardcoded renderer colors and custom titlebar/safe-area hacks.

## Scope

In scope:

- Eden sidebar layout/presentation only.
- Fixed header region with the four icon actions: settings, search, new note, sidebar toggle.
- Fixed bottom settings block with its own backing surface and top border/shadow treatment.
- Single scroll surface for the main sidebar content only.
- Gutter-based scrollbar spacing so the content does not become asymmetrically padded when the scrollbar appears.
- Collapsed-sidebar visibility rules for the top actions.
- Token-based hover styling for the icon buttons.

Out of scope:

- Any ARK/data/sync/schema/write-boundary changes.
- Navigation, search behavior, settings behavior, or note/object data changes.
- Broad refactors outside the sidebar shell/layout.
- Reintroducing or preserving the `Eden` title.

## Assumptions

- “Collapsed” refers to the existing sidebar hidden/collapsed state surfaced through the current Eden shell.
- The fixed bottom settings block is a visual/layout element, not a new settings model or route.
- The hover treatment must come from Kosmos visuals tokens/vars, not hardcoded hex/rgb values.

## Constraints and non-goals

- Do not change production data flow, ARK, sync, schema, or persistence.
- Do not introduce new sidebar navigation semantics.
- Do not rely on nested scroll regions for header, content, or settings.
- Do not use hardcoded renderer colors or custom chrome/titlebar code.
- Do not rename or broaden the sidebar task into unrelated visual cleanup.

## Acceptance Criteria

**AC1.** The sidebar has a fixed header area containing the sidebar toggle plus settings, search, and new note icon buttons; this header does not scroll.

**AC2.** The sidebar has a fixed bottom settings block; it remains visible/stable and has the intended backing surface plus top border/shadow effect when content scrolls beneath it.

**AC3.** Only the main sidebar content area scrolls. Header actions and the bottom settings block stay pinned and are never part of the scrolling region.

**AC4.** The `Eden` title is removed from the sidebar.

**AC5.** When the sidebar is collapsed, only the sidebar toggle remains visible; settings, search, and new note are hidden.

**AC6.** The sidebar action buttons use a hover background derived from Kosmos visuals tokens/vars, with no hardcoded color literals.

**AC7.** At small vertical heights, sidebar content scrolls instead of squeezing, overlapping, or becoming unreadable/unclickable.

**AC8.** The content area uses gutter-based scrollbar spacing so appearance/disappearance of the scrollbar does not create asymmetric padding or layout shift.

**AC9.** Visual verification is performed against a short-height sidebar state with enough content to force overflow, and the result is reported explicitly.

## Verification plan

- Run the narrow Eden UI check/typecheck available in this repo for sidebar/layout edits.
- Perform manual visual verification in the desktop shell with reduced window height and enough sidebar content to overflow.
- Confirm there is one main sidebar scrollbar, no squeezed/overlapping rows, and the fixed header/footer behavior matches the spec.
