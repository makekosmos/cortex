# Task Spec - Eden Anytype UX pass for object types

## Goal
Bring Eden's object-type experience materially closer to Anytype in visual hierarchy and interaction model for:
- object headers on note/game pages
- object type settings and editing flow

Also add demo Ark seed data so the new UI can be exercised with both `note_obj` and `game_obj`.

## Sources
- `sample/anytype-ts-develop/src/ts/component/page/elements/head/simple.tsx`
- `sample/anytype-ts-develop/src/ts/component/block/featured.tsx`
- `sample/anytype-ts-develop/src/ts/component/sidebar/page/type.tsx`
- `sample/anytype-ts-develop/src/ts/component/sidebar/preview.tsx`
- `sample/anytype-ts-develop/src/scss/component/headSimple.scss`
- `sample/anytype-ts-develop/src/scss/block/featured.scss`
- `sample/anytype-ts-develop/src/scss/component/sidebar/page/type.scss`
- `sample/anytype-ts-develop/src/scss/component/sidebar/preview.scss`

## Component map
- `ObjectTypesSettings.vue`: route-level composition surface only.
- `ObjectTypesSidebar.vue`: list/search/select of built-in and custom object types.
- `ObjectTypeEditor.vue`: editor shell with sections for identity, fields, and preview.
- `TypedHeader.vue`: object page header in Anytype-inspired visual style.
- `useObjectTypeDraft.ts`: draft state, preview data, schema/UI-schema conversion helpers.
- `seedArkObjectDemo.ts`: reusable seed generator for demo Ark DBs.

## Acceptance Criteria
- AC1: `TypedHeader.vue` no longer renders as a rounded "card" block; it uses an Anytype-like page-header hierarchy with icon/cover, title, description, and lightweight featured properties.
- AC2: Object type settings are visually reworked into a two-pane Anytype-like editor with a clearer type list, editing sections, and preview area.
- AC3: The object type editor logic is split out of the route container so `ObjectTypesSettings.vue` is primarily orchestration.
- AC4: A demo Ark seed script exists and can create test data for both `note_obj` and `game_obj`.
- AC5: Eden typecheck and production build pass after the UX refactor.
- AC6: No new mojibake is introduced in the touched Eden UI files or task artifacts.
