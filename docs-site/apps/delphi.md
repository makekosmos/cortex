# Delphi — задачи

::: tip Источник правды
`apps/delphi/AGENTS.md`, `apps/delphi/README.md`, `docs/DELPHI-LEGACY-DB-DECISION.md`
:::

Delphi — приложение для управления задачами в Kepler. Активный desktop-рантайм — Electron + Vue в `apps/delphi/ts`. Android/Swift код может существовать рядом, но desktop ARK-интеграция применяется только к Electron-приложению.

## ARK Runtime

- Канонический desktop sidecar — `ark-core-rpc` из `packages/ark-core/rust`.
- **Старый Delphi-specific Rust DB sidecar удалён.** Не пересобирать, не восстанавливать, не упаковывать, не использовать как fallback.
- Задачи Delphi хранятся как обобщённые ARK-объекты с `type_id = task_obj`.
- App-код обращается к ARK через `@kepler/ark` или существующую обёртку `electron/sidecar.ts` вокруг `ark-core-rpc`.
- На входе в приложение и при смене shared-space **legacy todos мигрируются в `task_obj`**. После миграции object-данные — источник правды.

## Текущая модель

- `objects` с `type_id = task_obj` — сами задачи.
- `object_types` — typed/schema metadata.
- `object_links` — связи.
- App access — через `@kepler/ark` / `ark-core-rpc`.

См. `docs/DELPHI-LEGACY-DB-DECISION.md`.

## Команды

Запуск из `apps/delphi/ts`:

```powershell
bun run build:ark:dev      # debug-сборка ark-core-rpc
bun run build:ark          # release-сборка ark-core-rpc
bun run dev                # build:ark:dev + Vite + Electron
bun run build              # build:ark + TypeScript + Vite + electron-builder
bun run test               # unit
bun run e2e                # Playwright
```

## Boundaries

- Renderer **только** через preload API.
- Electron main/preload экспонируют узкие IPC методы.
- Новые task writes идут в ARK `task_obj`, **не** в legacy todo таблицы.
- Тесты и smoke checks используют изолированные тестовые БД / temp app-data пути, **никогда** — main user ARK DB.

## Ключевые модули

- `apps/delphi/ts/electron/main.ts` — Electron main.
- `apps/delphi/ts/electron/sidecar.ts` — поднятие и владение `ark-core-rpc`.
- `apps/delphi/ts/shared/task-object-migration.ts` — стартовая миграция legacy → `task_obj`.
- `apps/delphi/ts/scripts/verifySharedArkTask.mjs` — verification скрипт shared task state.
- `apps/delphi/ts/e2e/shared-ark-task.spec.ts` — e2e shared task flow.
- `apps/delphi/ts/src/services/storage/task-object-migration.test.ts` — тест миграции.

## Mobile / Native

- `apps/delphi/kotlin` — Android-часть (отдельные правила, см. её `AGENTS.md`).
- Desktop ARK-решения отсюда **не** применяются к Kotlin, если задача явно не говорит обратное.

## Запрещено

- ❌ Восстанавливать или паковать `apps/delphi/ts/sidecar` (legacy).
- ❌ Использовать legacy todo таблицы как long-term fallback после миграции.
- ❌ Прямой SQL write в `objects` из app services (см. [Граница записи](/concepts/write-boundary)).
- ❌ Дефолт пути к user DB в тестах.

## Связанные документы

- `docs/DELPHI-LEGACY-DB-DECISION.md` — почему legacy sidecar удалён.
- [Модель данных ARK](/concepts/ark-objects).
- [@kepler/ark](/packages/kepler-ark).
