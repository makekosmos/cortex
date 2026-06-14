# Task Spec — Eden sidebar unified scroll and object-type icon presentation

## Task ID / Path

- `2026-06-14-eden-sidebar-scroll-icons`
- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/spec.md`

## Original task statement

> Task: Create a FULL_LOOP spec for this Eden sidebar UI/data presentation task. User request (Russian): "поработаем с eden. в сайдбаре сейчас есть 2 скроллящихся компонента, это недавние и объекты. Скроллиться должен сам контент сайдбара, а не внутренние его части. В недавнем можно увидеть в том числе объекты если недавно их открывал. Также в типах объектах отображаем все объекты и иконки им дай их собственные, цветные. У самих наших объектов (не страниц списков) иконки должны быть серыми. Используй duotone phosphore icons."
>
> Scope: Eden sidebar only unless evidence shows a small adjacent data mapping change is required. Include acceptance criteria, forbidden boundaries (no direct SQL/write boundary changes, no broad refactors), visual verification requirements, and minimal commands. Return spec path and concise summary. Do not implement.

## Context

Relevant files/docs inspected while freezing the spec:

- `products/eden/src/components/sidebar/EdenSidebar.vue` — sidebar currently renders `Недавние` and `Объекты` as separate groups, each with `overflow-y-auto kosmos-scroll`; icons currently come from `@lucide/vue` (`FileText`, `Shapes`, etc.).
- `products/eden/src/App.vue` — passes `recentSidebarEntries`, `noteTypes`, `currentEntry`, and navigation handlers into `EdenSidebar`.
- `products/eden/src/lib/typedNotes.ts` — `NoteType` has `icon` and `color` fields; `getNoteTypeCollectionName()` resolves collection/list page labels.
- `products/eden/src/lib/systemTypes.ts` — system type ids include note/person/image/game/journal/workout/exercise.
- `products/eden/package.json` — `@phosphor-icons/vue` is already available.
- `docs-site/apps/eden/ui.md`, `docs-site/apps/eden/data.md`, `docs-site/apps/eden/typed-notes.md`.
- `docs-site/agents/forbidden/ui.md`, `docs-site/agents/forbidden/eden.md`, `docs-site/concepts/write-boundary.md`.

## Scope

In scope:

- Eden sidebar presentation in `products/eden/src/components/sidebar/EdenSidebar.vue`.
- Removing nested independent scrolling from the `Недавние` and `Объекты` groups so the sidebar content area scrolls as one continuous surface.
- Keeping `Недавние` able to show recently opened entries of any Eden object type, including typed objects.
- Showing all Eden object types in the sidebar object-type/list section, with each object-type/list page using its own duotone Phosphor icon and colored treatment.
- Rendering actual object/page entries (for example items in `Недавние`) with grey/neutral icons, even when their type has a colored icon for the list page.
- A small adjacent icon-resolution/mapping helper is allowed if keeping all icon mapping inline in the component would be brittle.

Out of scope:

- Changing object storage, schemas, ARK sync behavior, or object-type persistence semantics.
- Reworking settings object-type editor UX beyond what is strictly required to display existing `NoteType.icon` / `NoteType.color` data safely.
- Refactoring Eden navigation, history, editor, search, settings, or typed-object pages.
- Replacing icon systems globally outside the Eden sidebar.
- Changing which entries are considered “recent” outside the existing sidebar recent-entry source unless evidence proves the current source filters typed objects incorrectly.

## Assumptions / ambiguity resolution

- “Типы объектов отображаем все объекты” means the sidebar’s object/list section should include every available `noteType` passed to `EdenSidebar` (system and custom), not only a subset.
- “Собственные, цветные иконки” applies to object type/list-page rows (e.g. collection pages like people/games/images), not to individual object/page rows.
- “У самих наших объектов ... иконки должны быть серыми” means actual `Entry` rows (not collection/list rows), including those in `Недавние`, use neutral grey icon styling. They may still use a type-appropriate icon shape if implemented, but color must remain neutral.
- Phosphor means `@phosphor-icons/vue` components with `weight="duotone"`; do not add a new icon dependency.
- Existing data colors from `NoteType.color` may be used if already present and safely validated/fallbacked; hardcoded renderer hex values must not be introduced.

## Constraints and forbidden boundaries

- No direct SQL writes and no renderer SQLite access.
- No ARK write-boundary changes, sync changes, schema migrations, or destructive migrations.
- No broad refactors or unrelated visual cleanup.
- User-facing text remains Russian.
- Use `@kosmos/visuals` / CSS variables / existing data-driven colors; do not introduce hardcoded renderer `#hex`/`rgb()` color constants.
- Keep titlebar/safe-area behavior untouched; continue using existing shell/sidebar surfaces.
- If event listeners are added (not expected), they must have cleanup.
- Do not use `git add -A`, `--no-verify`, destructive git operations, or release/version changes.

## Acceptance Criteria

**AC1.** In the Eden sidebar normal notes/type-collection screens, `Недавние` and `Объекты` no longer have their own independent vertical scrollbars; the sidebar content body scrolls as a single continuous area when content overflows.

**AC2.** The primary top actions and footer/settings action remain reachable and visually stable with long recent/object-type lists; no content is clipped or made unreachable by the single-scroll layout.

**AC3.** `Недавние` continues to render recently opened entries of all supported entry/object types, including typed objects recently opened by the user; the change must not filter recent items down to only plain notes.

**AC4.** The sidebar object-type/list section renders all `props.noteTypes` available to `EdenSidebar` (system and custom) in a deterministic order and does not drop custom or system types because of icon/color availability.

**AC5.** Every object-type/list row in the sidebar uses a duotone Phosphor icon (`@phosphor-icons/vue`, `weight="duotone"`) with a distinct colored treatment derived from the type’s configured presentation when available, and a stable fallback when not.

**AC6.** Actual object/page rows, including rows in `Недавние`, use neutral grey icon styling rather than the colored object-type/list styling. This distinction is visible when a recent item is a typed object whose collection row is colored.

**AC7.** Existing sidebar navigation actions still work: opening a recent entry, opening an object-type collection, creating a note, creating an object type, opening search, and opening settings.

**AC8.** The implementation remains within Eden sidebar presentation/mapping scope and introduces no ARK write-boundary, schema, sync, or broad navigation changes.

**AC9.** Visual behavior is verified with enough data to force overflow: multiple recent entries plus enough object types to exceed sidebar height. Verification must explicitly check that there is one sidebar content scrollbar and no inner scrollbars on `Недавние` / `Объекты`.

## Verification plan

Minimal commands/checks for the implementation worker:

1. `rtk bun run --cwd products/eden test:unit` if sidebar/icon helper logic gets unit coverage or existing Eden unit tests are affected.
2. `rtk bun run products:build` or the narrowest available Eden/extension build command that compiles `products/eden` through the desktop extension pipeline.
3. `rtk bun run ark:guard:writes` only if any touched file is near Eden data/API or ARK boundary code; expected not needed for pure sidebar-only changes, but safe if scope expands.

Manual/visual verification required:

- Launch Eden in the desktop shell or available local dev flow.
- Create/use a dataset with enough recent entries and object types to overflow the sidebar.
- Confirm the sidebar content scrolls as one area; `Недавние` and `Объекты` do not show separate scrollbars.
- Confirm recent typed objects are present when recently opened and have grey/neutral icons.
- Confirm object-type/list rows are all present and use colored duotone Phosphor icons.
- Report explicitly if visual verification could not be run.

## Notes

This is a FULL_LOOP task because it affects Eden UI layout plus presentation mapping for typed object data. The frozen scope is intentionally narrow: sidebar-only unless a tiny shared icon resolver is necessary to avoid duplicating brittle mappings.
