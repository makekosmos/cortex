# Sidebar Drag Layer

## Goal

Вернуть draggable-область на sidebar в `@kosmos/visuals`, но держать её фоновым слоем: под кнопками, ссылками и resize-handle.

## Acceptance Criteria

- AC1: Sidebar снова имеет рабочую drag-область для окна.
- AC2: Drag-layer не перехватывает sidebar-кнопки и project links.
- AC3: Drag-layer не мешает resize-handle справа.
- AC4: Изменение ограничено shared sidebar-слоем и не требует app-local hacks.

## Verification Plan

- Проверить diff в `packages/kosmos-visuals/components/Sidebar.vue`
- Проверить `git diff -- packages/kosmos-visuals/components/Sidebar.vue`
- Проверить `vue-tsc`/typecheck для dashboard как smoke-потребителя shared sidebar
