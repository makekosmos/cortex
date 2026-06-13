# UI запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

## UI

- ❌ Английский язык в UI приложений (placeholder'ы, лейблы, кнопки, эмпти-стейты, заголовки). User-facing — только русский. Английский OK для technical id'ов (`task_obj`, `time_entry_obj`).
- ❌ Hardcoded `#hex`, `rgb()`, кастомные шрифты в renderer-коде. Все цвета / радиусы / шрифты — через `var(--*)` из `@kosmos/visuals`.
- ❌ Свой titlebar / safe-area код. Всегда через `<DesktopChrome>` + `<DesktopContentSurface>`.
- ❌ Nested interactive elements: `role="button"` (или любой другой interactive role) на `<span>` / `<div>` **внутри** `<button>`. HTML это запрещает; screen reader'ы collapse'ят в одну кнопку и inner action становится недоступным с клавиатуры. Решение — два sibling `<button>` в композитной обёртке (см. `DateChip.vue` после 2026-05-18 фикса).
- ❌ Outside-click listener'ы через nested `watch(..., { once: true })` для cleanup'а. Паттерн ломается при quick open→close→open: новый handler регистрируется до того как старый отпишется. Используй symmetric `watch(isOpen, (val) => val ? addEventListener : removeEventListener)` + `onBeforeUnmount → removeEventListener` (mirror `ContextMenu.vue`).
- ❌ `addEventListener` в `onMounted` без соответствующего `removeEventListener` в `onBeforeUnmount`. Component re-mount (HMR, route navigation) накапливает duplicate listeners на `document` / `window`.
- ❌ `e.key === "<латинская буква>"` / `event.key === "a"` (и т.п.) для Ctrl/Cmd-shortcut'ов. **Раскладка обязана быть layout-agnostic**: на русской раскладке та же физическая клавиша возвращает `"ф"`, и Ctrl+A ловится только на EN. Используй **`e.code === "KeyA"`** (физическая клавиша). Аналогично `KeyC`, `KeyX`, `KeyS`, `KeyK`, `KeyN`, `KeyZ`. Исключение — non-letter keys (`Enter`, `Escape`, `ArrowUp`, цифры, F1-F12): для них `e.key` valid, потому что не зависит от alpha layout. Подтверждение — `docs-site/concepts/eden-zen-mode.md` (chord `Ctrl+K Z` через `e.code`) и regression test `tests/e2e/eden-clipboard-markdown.spec.ts`.
