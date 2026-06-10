# Для AI-агента — старт работы

::: tip Прочитай это ПЕРВЫМ
Эта страница заменяет тонкие `AGENTS.md` / `CLAUDE.md` в корне. Полный контекст репо — в этом сайте документации. Все правила в одном месте.
:::

Ты работаешь в монорепо **Kosmos** (после brand swap 2026-05-14). Перед любым изменением кода обязательно сверься с разделами ниже. Если задача нетривиальна — иди по [Proof loop](/concepts/proof-loop).

## За 30 секунд

- **Kosmos** = монорепо / экосистема для личного софта. Bun workspaces + Cargo workspace.
- **Kepler** = имя лаунчера (`platform/desktop/`, npm name `kepler-shell`) и его shared backend (`platform/runtime/`).
- **ARK** = общий Rust+SQLite рантайм (`core/ark/crates/ark-core`, бинарь `ark-core-rpc`).
- **Apps** говорят с ARK **только** через `@kosmos/ark` или `ark_core::db` (Rust direct writers).
- **Прямые SQL writes в ARK** из app services — **запрещены**.
- **Apps интегрируются с лаунчером через command bus** (apps регистрируют commands, Kepler invoke'ает).
- **Тесты** — только на изолированных БД.
- **Substantial-правки** — через `.agent/tasks/<DATE>-<slug>/` proof loop.

## Что должно сработать прежде, чем ты начнёшь редактировать код

Прочитай в указанном порядке:

1. **[Архитектура](/concepts/architecture)** — общая картина.
2. **[Системные требования](/concepts/system-requirements)** — что нужно для запуска и сборки (Windows-only сейчас, x64, Node 20+, Rust 1.80+, Bun).
3. **[Модель данных ARK](/concepts/ark-objects)** — что за таблицы и типы.
4. **[Граница записи в ARK](/concepts/write-boundary)** — что можно, что нельзя.
5. **[Изоляция тестовых БД](/concepts/test-isolation)** — как писать тесты.
6. **[Proof loop](/concepts/proof-loop)** — как оформлять substantial-задачи.
7. **[Запреты и гварды](/agents/forbidden)** — список «никогда».
8. **[Чек-листы по областям](/agents/checklists)** — что прогнать перед сдачей.

## Принципы работы

### 1. Не угадывай — читай источник

Перед правкой в `apps/<name>` прочитай `apps/<name>/AGENTS.md`. Перед правкой в data-слое — `docs/ARK-READONLY-SQL-BOUNDARY.md` и [Граница записи](/concepts/write-boundary).

### 2. Меньший defensible diff

Не «попутно отрефактори». Делай только то, что в задаче.

### 3. Не добавляй лишнее

- Не добавляй обработку ошибок для случаев, которые не могут случиться.
- Не добавляй fallback'и «на всякий случай».
- Не добавляй комментарии, объясняющие **что**. Имена и так это делают.
- Не добавляй feature flags, когда можно просто изменить код.

### 4. Прогоняй гварды

После любой правки в data-слой:

```powershell
bun run ark:guard:writes
```

После любой substantial-правки:

```powershell
bun run ark:smoke
```

### 5. Не клейми «готово» если AC не PASS

Если задача через proof loop — каждый AC должен быть `PASS` в `evidence.md`. Не пиши «готово» в чате, пока это не так.

### 6. Failing тесты исправляются всегда

Если в ходе работы обнаружен failing test / regression / баг — исправляется обязательно. Не важно, был ли он до моих правок или нет. Либо в текущем коммите (если по смыслу подходит), либо отдельным коммитом / proof loop'ом. «Pre-existing» — не оправдание.

### 7. Калибруй оценки времени

Перед тем как назвать пользователю срок («займёт N часов»), сверься со skill'ом `estimate-calibration` (`~/.claude/skills/estimate-calibration/`): прочитал log.jsonl прошлых prediction vs actual → скорректировал gut estimate → записал prediction до начала → закрыл row после AC PASS. Подробно — [Estimation](./estimation).

### 8. Тесты пишутся по стандартному паттерну

E2e в headless mode, universal extension contract через `manifest.tests`, per-app spec только для нетривиальных UI flow'ов. Полный гайд — [Testing](./testing).

## Карта приложений и пакетов

::: tip STATUS.md — always-current snapshot
Корневой `STATUS.md` хранит актуальный snapshot состояния проекта (что работает, что в работе, что сломано). Перед началом substantial-задачи открой его — карта ниже описывает «где что», а `STATUS.md` — «что сейчас в каком состоянии». Обновлять `STATUS.md` нужно, когда меняется статус приложения или появляется/исчезает заметная багу/фича.
:::

Когда пользователь упоминает имя — ты должен моментально знать, где это.

| Имя                       | Где                                                                             | Что                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Eden**                  | `products/eden`                                                                 | заметки и дневник — Vue-extension в Kepler shell (TipTap editor). Phase 6.0 + 6.0.A + **6.1 (2026-05-19)** done. Standalone `apps/eden/ts/` удалён. Phase 6.1: system type «Дневник» + ISO `YYYY-MM-DD` title; static open-команды `eden:note:open-today` / `eden:note:create`; zen-mode chord `Ctrl+K Z` + acrylic backdrop в zen; titlebar page-title + native window controls через Electron `titleBarOverlay`; char counter; ПКМ-меню «Удалить» через `ContextMenu`; иконки Lucide через `iconResolver.ts` (anytype-папка удалена); layout-agnostic hotkeys через `e.code`. |
| **Delphi**                | `products/delphi`                                                               | задачи — Vue-extension в Kepler shell                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **Arrancador**            | `incubator/arrancador`                                                          | игровая библиотека — Vue-extension                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **Dashboard**             | `platform/desktop/src/views/Dashboard*.vue` + `platform/desktop/src/dashboard/` | встроенный ARK browser: sidebar по типам + таблица объектов. До 2026-05-14 был extension'ом; старый dashboard-extension больше не active-tree path. 2026-05-15 убран welcome screen — single DB per user.                                                                                                                                                                                                                                                                                                                                                                       |
| **Akasha**                | `incubator/akasha`                                                              | EPUB-читалка — Vue-extension. Launcher command `akasha:open`; состояние v1 локально в `extensions-data/akasha`, без ARK writes. Старый Rust/GPUI prototype вынесен в приватный `ksanrse/akasha-gpui`.                                                                                                                                                                                                                                                                                                                                                                           |
| **Kepler Backend**        | `platform/runtime`                                                              | Rust-сервис: command bus host + WS server + встроенный `usage_tracker` модуль (после Phase E2)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **Extension host**        | `platform/desktop/electron/extension-host.ts` + `extensions/<id>/`              | Loader Vue-бандлов как extension windows внутри Kepler shell. Manifest + `openExtension(id)` + dev mode (HMR). См. [Extension host](/concepts/extension-host), [Extension dev mode](/concepts/extension-dev-mode).                                                                                                                                                                                                                                                                                                                                                              |
| **Command bus**           | `platform/runtime/src/command_bus.rs` + `@kosmos/ark` `commands` namespace      | In-memory registry команд + WS-операции `commands.{register,unregister,list,invoke}` + события `command_invoked` / `commands_changed`. См. [Command bus](/concepts/command-bus).                                                                                                                                                                                                                                                                                                                                                                                                |
| **Digital Cave**          | `apps/digital-cave`                                                             | focus-блокер (TBD, имя зарезервировано)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **Kerux**                 | `apps/kerux`                                                                    | голосовой ввод по хоткею, faster-whisper / Groq Whisper-v3 (TBD, имя зарезервировано)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **ark-service (Android)** | `incubator/mobile/ark-service`                                                  | Android Room ContentProvider для `incubator/mobile/delphi` (отдельно от desktop ARK)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **ark-core**              | `core/ark/crates/ark-core/rust`                                                 | Rust runtime + ark-core-rpc bin                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **@kosmos/ark**           | `core/ark/packages/ark`                                                         | TS SDK                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **@kosmos/visuals**       | `packages/visuals`                                                              | UI токены, тема, компоненты                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **ark-relay-server**      | `services/relay-reference`                                                      | WebSocket relay (опционально, для NAT)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **kepler-watcher**        | `platform/native-services/kepler-watcher`                                       | watcher-демон над `core/ark/crates/ark-core`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **usage-tracker**         | `platform/runtime/src/usage_tracker/`                                           | модуль внутри kepler-backend (был standalone до Phase E3; старый standalone usage-tracker больше не active-tree path)                                                                                                                                                                                                                                                                                                                                                                                                                                                           |

## Что считается substantial (нужен proof loop)

- Новая фича приложения.
- Новый ARK endpoint в `ark-core-rpc` или метод в `@kosmos/ark`.
- Изменение схемы SQLite.
- Изменение sync-протокола.
- Изменение write-boundary (правил доступа к данным).
- Нетривиальный багфикс (затрагивающий несколько файлов).
- Архитектурное решение (требует ADR в `docs/`).

## Что НЕ substantial

- Опечатки.
- Локальное переименование переменной.
- Косметика README / комментариев.
- Edit-level правка одной строки в UI.
- Обновление зависимости patch-версии.

Для не-substantial proof loop **не нужен**. Просто правь.

## Дальше

- [Чек-листы по областям](/agents/checklists) — что прогнать перед сдачей в каждой области.
- [Запреты и гварды](/agents/forbidden) — список «никогда».
- [Шаблоны спецификаций](/agents/spec-templates) — типовые `spec.md` для proof loop.
- [Testing](/agents/testing) — как писать e2e (headless mode, universal contract, ловушки).
- [Estimation](/agents/estimation) — калибровка оценок времени через skill.
- [Manual tests waiting](/agents/manual-tests-pending) — твой TODO-чек-лист визуальных проверок, не покрытых автоматическими тестами.

Полный справочник правил — [Правила репозитория](/reference/rules).
