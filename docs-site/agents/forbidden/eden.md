# Eden запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

### Eden

- ❌ Возврат к ripgrep / Tantivy / Heart Rust sidecar. Search 100% через ARK FTS5 (`search_objects`).
- ❌ Возрождение `apps/eden/ts/` или standalone Eden.exe. Удалены в Phase 6.0.A.
- ❌ Возврат Hevy fitness integration в Eden. Замена — Olympia (отдельное приложение, ещё не реализовано).
- ❌ Возврат UI для code lint/format. Code-tools UI удалены в 6.0.A; TipTap CodeBlock + lowlight (синтакс highlight) остаются.
- ❌ Vault picker UI / welcome screen / multi-vault. Single ARK DB per user после удаления spaces (2026-05-15).
- ❌ Прямое использование `window.kepler.ark.request` из Eden компонентов и `store/`. Только через `kepler-api-shim` (или `edenApi.ts` фасад над ним) — это единственный мост, чтобы code review мог локально проверить ARK границу.
- ❌ Возврат ручных `--titlebar-height` / `--titlebar-left-safe-area` костылей.
- ❌ Использование `vue-router` для titlebar history controls (нужна локальная история Eden).
- ❌ Deep import shared компонентов вместо public API `@kosmos/visuals`.
- ❌ Удаление lazy-load Editor.vue (`defineAsyncComponent`). Main bundle Eden должен оставаться < 800KB.
- ❌ `await props.onSave(...)` в `Editor.vue` без try/catch. Throw'и из onSave (network error, ARK недоступен) при отсутствии catch'а превращают autosave в silent retry storm — `lastPersisted*` не обновляется, autosave таймер ретраит каждые 800ms бесконечно, пользователю никаких индикаторов. На failure — выставить `saveConflict` с human-readable текстом.
- ❌ `void save()` в `onBeforeUnmount` без `.catch(...)`. Promise rejected после unmount'а компонента → unhandled rejection. Component-instance-aware error handling не сработает (компонент уже размонтирован).
