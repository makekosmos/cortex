# 2026-05-17 universal-extension-contract

## Context

Сейчас Playwright e2e (`tests/e2e/*.spec.ts`) — это per-app suites: `launcher.spec.ts`, `horologion.spec.ts`, `horologion-pomodoro.spec.ts`, `delphi-tasks.spec.ts`, и т.д. Каждый spec знает app-specific селекторы, fixtures и flow. При добавлении нового extension'а — нужно писать аналогичный набор с нуля.

Решение пользователя 2026-05-17: **унифицировать архитектурный baseline** через manifest-driven contract spec. Extension в своём `manifest.json` объявляет минимальные «ручки» (команды + smoke ARK object type), а универсальный e2e spec автоматически проверяет:

1. Extension загружается без crash'а.
2. Объявленные команды появляются в `commands.list` после boot'а.
3. ARK round-trip объекта объявленного типа работает (upsert → get → cleanup).

Это покрывает архитектурный инвариант («extension не сломан как extension»). Per-app UI flow специфики (TipTap рендерится, Pomodoro переключается, Delphi sidebar пустой и т.п.) — отдельные per-app spec'и.

Этот proof loop — Phase 6.0.5: universal contract + минимальный per-app `eden.spec.ts` (раньше отсутствовал, так как Eden был standalone .exe).

## Scope

В задаче:

- Расширить `shell/electron/extension-host.ts → ExtensionManifest` опциональным полем:
  ```ts
  tests?: {
    commands?: string[];        // commands.list содержит все из списка после boot extension'а
    smoke?: {
      objectType: string;       // type id для ARK round-trip
      sample?: {
        title?: string;
        content?: unknown;
        props?: Record<string, unknown>;
      };
    };
  };
  ```
- Заполнить `tests`-поле во всех 4 extension manifest'ах (`extensions/eden`, `extensions/horologion`, `extensions/delphi`, `extensions/arrancador`):
  - eden: `commands: ["eden:note:create", "eden:note:search"]`, `smoke.objectType: "note_obj"`
  - horologion: `commands: ["horologion:pomodoro:25", "horologion:pomodoro:50", "horologion:stopwatch:start"]`, `smoke.objectType: "time_entry_obj"`
  - delphi: `commands: ["delphi:task:create", "delphi:task:today"]`, `smoke.objectType: "task_obj"`
  - arrancador: `commands: []`, `smoke.objectType: "game_obj"`
- Создать `tests/e2e/extensions-contract.spec.ts`:
  - Discover all `extensions/*/manifest.json` через `fs.readdirSync`.
  - Для каждого extension'а с непустым `tests` блоком — `test.describe("extension contract: ${id}")` с тестами:
    - **boot**: открыть extension через `kepler.commands.invoke("${id}:open")` (через launcher window's `window.kepler.commands.invoke`); дождаться появления extension window; убедиться что не destroyed после 1.5 сек.
    - **commands**: если `tests.commands.length > 0` — через extension window's `window.kepler.ark.request("commands.list")` получить commands, убедиться что все объявленные присутствуют.
    - **smoke**: если `tests.smoke` определён — через extension window's `window.kepler.ark.request("upsert_object", {object: {...}})` создать sample, прочитать через `get_object`, убедиться что title/typeId совпадают. В конце — cleanup через `delete_object`.
- Создать `tests/e2e/eden.spec.ts`:
  - Открыть Eden через `eden:open` команду
  - Дождаться extension window + первичный render
  - Проверить что `window.api` shim установлен (т.е. shim из `installKeplerApiShim()` отработал)
  - Через `window.api.saveEntry` создать заметку с уникальным title
  - Reopen extension, через `window.api.listEntries` проверить что заметка persisted
  - Cleanup через `window.api.deleteEntry`
- `playwright.config.ts` — без изменений (single worker, fullyParallel:false уже корректны).

Не в задаче:

- Cleanup stale ACL lock-файлов в `tests/.e2e/*/kepler.lock.json` (нужен admin shell, это **pre-existing environmental** проблема пользователя — см. Loop A problems.md для рекомендации).
- Per-app UI deep tests (TipTap WYSIWYG verification, pomodoro timer progression и т.п.) — отдельная задача.
- Test smoke matrix update (docs-site/reference/smoke-matrix.md уже обновлён в Loop A с упоминанием `tests/e2e/eden.spec.ts`).

## Acceptance Criteria

**AC1.** `shell/electron/extension-host.ts → ExtensionManifest` имеет опциональное поле `tests` с указанной shape.

**AC2.** Все 4 extension manifest'а (`extensions/{eden,horologion,delphi,arrancador}/manifest.json`) имеют валидный `tests` блок.

**AC3.** `tests/e2e/extensions-contract.spec.ts` существует, импортирует `launchKepler`, дискаверит manifest'ы, генерирует test'ы для каждого extension'а с `tests` блоком.

**AC4.** `tests/e2e/eden.spec.ts` существует — открытие Eden, проверка `window.api`, write/read note, cleanup.

**AC5.** `bun run --cwd shell typecheck` — clean.

**AC6.** `bun run --cwd shell build:js` — extensions + main + renderer собираются без ошибок.

**AC7.** Manifest parsing в `loadExtensionManifest()` (`shell/electron/extension-host.ts`) корректно деpattern-маtch'ит новое поле `tests` — runtime регрессий не появилось (manifest deserialize не падает на наличии `tests`).

**AC8** (PENDING_OPERATOR). `bun run test:e2e` после cleanup'а stale ACL lock-файлов в `tests/.e2e/` (см. Loop A `problems.md`) — `extensions-contract` и `eden` spec'и зелёные. Прогон оператором; этот AC не блокирует proof-loop closure для AC1-AC7.

## Verification commands

См. AC. Спеки прогоняются `bun run test:e2e --grep "extension contract|eden"`. Если ACL lock-файлы блокируют — оператор cleanup'ит, прогоняет повторно.

## Out of scope decisions

- Smoke `tests/.e2e/*/kepler.lock.json` cleanup — pre-existing environmental, отдельная одноразовая задача оператора.
- Расширение manifest.tests дополнительными hooks (например `tests.preInvoke` callback в TS) — overengineering для Phase 6.0.5, добавится по необходимости.
- Поддержка `tests` поля в `extension-installer.ts` (CLI install / publish UI) — не нужна сейчас, поскольку manifest.tests — это test-only metadata, не runtime ARK contract.
