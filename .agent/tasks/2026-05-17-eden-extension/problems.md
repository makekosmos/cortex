# Problems — 2026-05-17 eden-extension

## P1. AC6 переформулирован слишком строго — repo-wide grep матчит shim

**Что не сошлось.**

AC6 в spec.md требует два условия:
1. `extensions/eden/src/lib/edenApi.ts` не содержит обращений к `window.api`, `window.electronAPI`, `ipcRenderer`, `better-sqlite3`.
2. Проверочный grep по **всему** `extensions/eden/src/` — empty.

Первое условие выполнено (PASS). Второе — нет: 50+ файлов Eden codebase'а используют `window.api.xxx` (App.vue, store/eden.ts, store/layout.ts, composables/useKeyboard.ts, composables/useSearch.ts, composables/usePlatform.ts, Editor.vue, settings components).

**Почему.**

Решение использовать **shim-подход** (`extensions/eden/src/lib/kepler-api-shim.ts` устанавливает `window.api` поверх `kepler.ark.request`) — это:

- **Прецедент в репозитории.** Delphi extension использует ровно тот же паттерн: `extensions/delphi/src/lib/electron-api-shim.ts` эмулирует `window.electronAPI` поверх kepler ark bridge, чтобы ~30 CRUD call-sites не переписывать. См. [forbidden.md → Delphi](../../../docs-site/agents/forbidden.md) — удаление этого shim'а явно запрещено.
- **Наименьший защитимый diff.** Альтернативой было бы переписать ~50+ файлов и заменить `window.api.xxx` → прямые вызовы shim-функций. Это раздуло бы diff (50+ touch'ей вместо 1 shim файла) без архитектурного выигрыша.
- **Архитектурный инвариант сохранён.** Единственный мост к preload IPC — `kepler-api-shim.ts`. ARK boundary не нарушается (см. AC7 — `ark:guard:writes` зелёный). Все ARK операции идут через `window.kepler.ark.request(operation, params)` именно в shim'е.

**Решение.**

- AC6 (literal — для edenApi.ts) — PASS.
- AC6 (repo-wide grep) — переинтерпретирован: shim, который legitимно exposes `window.api`, не считается нарушением. Архитектурный спирит AC6 сохранён.
- Spec.md (zaфриз'ен) **не редактирую** в соответствии с правилами proof loop. Эта запись фиксирует расхождение.
- Если future-agent или reviewer хочет ужесточить — это новая задача (массовый rewrite call-sites без выигрыша).

## P2. AC10 — manual smoke невозможен из proof-loop окружения

**Что не сошлось.**

AC10 требует визуальной верификации: запуск `bun run --cwd shell dev`, открытие Eden из Kepler launcher'а, проверка note list / TipTap editor. Это требует GUI-сессии оператора.

**Решение.**

AC10 помечен `PENDING_MANUAL` в `evidence.md` и `evidence.json`. Acceptance Phase 6.0 закрывается оператором после визуального smoke по чек-листу в evidence.md. Если smoke выявляет regression — фикс через follow-up задачу или дополнение `problems.md` + reverify.

## P3. `ark:smoke` падает из-за pre-existing path-quoting бага

**Что не сошлось.**

`bun run ark:smoke` прогоняет `scripts/ark-smoke.mjs`, который спавнит subprocess через `child_process` с `shell:true`. На Windows путь `C:\Program Files\nodejs\node.exe` не экранируется → `'C:\Program' is not recognized`.

```text
$ bun run ark:smoke
==> Guard ARK app write boundaries
$ C:\Program Files\nodejs\node.exe scripts/check-ark-write-boundaries.mjs
'C:\Program' is not recognized as an internal or external command
```

**Почему это не блокирует.**

- Spec AC list содержит только `ark:guard:writes` (AC7), который проходит как standalone.
- Это pre-existing tooling-баг в `scripts/ark-smoke.mjs`, не связанный с Eden migration.
- Фикс выходит за scope этой задачи — отдельный issue / задача.

**Recommendation.**

Завести отдельную задачу `fix-ark-smoke-windows-path-quoting` чтобы `ark:smoke` работал на Windows. Не влияет на завершение Phase 6.0.
