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
bun run build:js           # build:ark + TS + Vite (без установщика)
bun run build              # build:js + electron-builder --win nsis (финальный NSIS one-click)
bun run package:dir        # unpacked desktop bundle
bun run test               # unit
bun run e2e                # Playwright
```

Артефакты после `build` лежат в `apps/delphi/ts/release/`:

- `Delphi Setup <version>.exe` — финальный NSIS one-click установщик (см. [конвенцию сборки релизов](/reference/commands#конвенция-сборки-релизов)).
- `win-unpacked/Delphi.exe` — распакованное приложение (доступно после `package:dir`).

Версия берётся из `package.json` → `version` (текущая `0.0.2`).

## Иконка

Источник — `apps/delphi/ts/build/icon.png` (минимум 512×512, рекомендуется ≥1024×1024 PNG).
Pipeline:

- `package.json → build.win.icon` указан явно на `build/icon.png`.
- `build/afterPack.cjs` (hook electron-builder) конвертирует PNG → ICO через `png-to-ico`
  и встраивает иконку + version-string метаданные в `Delphi.exe` через `rcedit`.
  Это нужно, потому что `win.signAndEditExecutable: false` отключает встроенный
  rcedit electron-builder (workaround под падение winCodeSign symlinks без Developer Mode).
- `extraResources` копирует `build/icon.png` в `resources/icon.png` packaged-сборки;
  `electron/main.ts` использует её для `BrowserWindow.icon` (taskbar / тайтлбар).
- В dev иконка читается из `apps/delphi/ts/build/icon.png` напрямую через `resolveIconPath()`.

Чтобы обновить иконку — замени `build/icon.png` и перезапусти `bun run build`.
Кэшированный `build/icon.ico` afterPack перегенерит, если PNG новее.

Это стандарт для всех Electron-приложений Kepler — см. [Структура репо → Иконки](/guide/layout#иконки-приложений).

## Boundaries

- Renderer **только** через preload API.
- Electron main/preload экспонируют узкие IPC методы.
- Новые task writes идут в ARK `task_obj`, **не** в legacy todo таблицы.
- Тесты и smoke checks используют изолированные тестовые БД / temp app-data пути, **никогда** — main user ARK DB.

## Ключевые модули

- `apps/delphi/ts/electron/main.ts` — Electron main. IPC `ark:listDelphiTasks`, `ark:upsertDelphiTask`, `ark:deleteDelphiTask`, `ark:listTimeEntries` (читает `time_entry_obj` для ProjectPage биллинга).
- `apps/delphi/ts/electron/sidecar.ts` — поднятие и владение `ark-core-rpc`.
- `apps/delphi/ts/shared/task-object-migration.ts` — стартовая миграция legacy → `task_obj`.
- `apps/delphi/ts/shared/task-ark.ts` — мапперы `TodoItem ↔ task_obj` (propsJson). Здесь же читается/пишется `billable` / `price`.
- `apps/delphi/ts/src/components/QuickEntry.vue` — обёртка над `QuickEntryPanel` из `@kepler/visuals`. Передаёт `defaultScheduledDate=today` если открыто со страницы `/today`, `defaultProjectId` если со страницы проекта.
- `apps/delphi/ts/src/components/projects/ProjectCreateDialog.vue` — диалог нового проекта, toggle «Оплачиваемый» + поле «Бюджет».
- `apps/delphi/ts/src/pages/ProjectPage.vue` — поллит `ark:listTimeEntries` каждые 15s, агрегирует billable секунды по задаче, считает $/час из `project.price`.
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

## UX модель задач

Текущая модель форм / списков:

- **Sidebar:** только «Входящие» и «Сегодня» как top-level nav. Календарь/Неделя удалены — это не приоритет.
- **QuickEntry** (⌘N / Ctrl+N) — модалка над content-областью (не перекрывает titlebar и sidebar). Поля: title, notes, дата (через `<DateChip>` — попап с `<Calendar>` из `@kepler/visuals`, native browser date picker не используем), проект (dropdown реальных проектов + «Входящие»), `billable` toggle + опциональный `price`.
- **TodoRow** — клик разворачивает inline-форму (title / notes / дата / billable / price), правый клик открывает `<ContextMenu>` с пунктом «Удалить». Inline-кнопка delete не используется.
- **ProjectPage** — кроме списка задач показывает: бейдж «оплачиваемый + бюджет», суммарное оплачиваемое время по задачам проекта, расчётный `$/час`.

## Биллинг

Реализован минимальный flow «оплачиваемая задача + опциональная цена» + наследование от проекта.

### `task_obj.propsJson` (актуальная схема)

```ts
{
  billable: boolean,     // источник правды — стор задачи (Delphi)
  price: number | null,  // опциональная сумма за задачу (фикс-цена)
  // ... остальные поля task (description, priority, dates, и т.д.)
}
```

Сериализуется в `apps/delphi/ts/shared/task-ark.ts → todoToArkTaskObject/arkTaskObjectToTodo`. Старые задачи без полей читаются как `billable=false, price=null` (back-compat).

### `Project` (Delphi)

```ts
type Project = {
  // ...
  billable: boolean,     // помечает весь проект как оплачиваемый
  price?: number | null, // общий бюджет проекта (опц.)
}
```

### Наследование

При создании задачи в проекте `billable` авто-подставляется из `project.billable` (через `QuickEntryPanel` — поле `QuickEntryProject.billable`). Пользователь может переопределить toggle'ом до сохранения. После сохранения значения независимы — задача хранит свой флаг.

### Распределение по time entries

[Horologion](/apps/horologion) пишет `time_entry_obj` с `propsJson.taskId` и `propsJson.billable`. Delphi читает их через IPC `ark:listTimeEntries` (`apps/delphi/ts/electron/main.ts`) и в `ProjectPage`:

- Σ billable секунд по задаче → chip с часами рядом со строкой.
- Σ billable секунд по всем задачам проекта → общий часовой счётчик.
- Если у проекта задан `price` — `$/час = price / Σ(billable_hours)` (на месте).

Сам Horologion цены не показывает — только пишет `billable` флаг. Все денежные расчёты идут в Delphi (на страницах) или в Dashboard.

## Связанные документы

- `docs/DELPHI-LEGACY-DB-DECISION.md` — почему legacy sidecar удалён.
- [Модель данных ARK](/concepts/ark-objects).
- [Horologion](/apps/horologion) — трекер времени, который привязывается к Delphi-задачам.
- [@kepler/ark](/packages/kepler-ark).
