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
bun run build              # build:js + electron-builder --win msi (финальный MSI)
bun run package:dir        # unpacked desktop bundle
bun run test               # unit
bun run e2e                # Playwright
```

Артефакты после `build` лежат в `apps/delphi/ts/release/`:

- `Delphi <version>.msi` — финальный установщик (Windows Installer, per-machine).
- `win-unpacked/Delphi.exe` — распакованное приложение (доступно после `package:dir`).

Версия берётся из `package.json` → `version` (текущая `0.0.2`). MSI — единый формат
дистрибуции для всех desktop-приложений Kepler, см. [конвенцию сборки релизов](/reference/commands#корневые).

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

## TODO — биллинг

Планируется (не сделано): добавить опциональные поля в `propsJson` `task_obj` для расчёта дохода:

```ts
// task_obj.propsJson — потенциальные поля
{
  priceModel?: 'fixed' | 'hourly',
  price?: number,        // fixed: общая сумма за задачу
  hourlyRate?: number,   // hourly: ставка в час
  currency?: 'USD' | 'RUB' | 'EUR' | ...,
}
```

Time entries из [Horologion](/apps/horologion) ссылаются на `task_obj` через `object_link` (`linkType='for-task'`). Расчёт `$/час`:

- **fixed model**: `price / Σ(time_entries.duration where billable)` — фактический $/час за результат.
- **hourly model**: `Σ(time_entries.duration where billable) × hourlyRate` — заработано.

Считать в Dashboard report или в самой Delphi на странице задачи. **Сам Horologion цены не показывает** — только пишет `time_entry.propsJson.billable`.

## Связанные документы

- `docs/DELPHI-LEGACY-DB-DECISION.md` — почему legacy sidecar удалён.
- [Модель данных ARK](/concepts/ark-objects).
- [Horologion](/apps/horologion) — трекер времени, который привязывается к Delphi-задачам.
- [@kepler/ark](/packages/kepler-ark).
