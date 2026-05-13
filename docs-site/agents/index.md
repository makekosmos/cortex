# Для AI-агента — старт работы

::: tip Прочитай это ПЕРВЫМ
Эта страница заменяет тонкие `AGENTS.md` / `CLAUDE.md` в корне. Полный контекст репо — в этом сайте документации. Все правила в одном месте.
:::

Ты работаешь в монорепо **Kepler**. Перед любым изменением кода обязательно сверься с разделами ниже. Если задача нетривиальна — иди по [Proof loop](/concepts/proof-loop).

## За 30 секунд

- **Kepler** = монорепо для личного софта. Bun workspaces.
- **ARK** = общий Rust+SQLite рантайм (`packages/ark-core`, бинарь `ark-core-rpc`).
- **Apps** говорят с ARK **только** через `@kepler/ark` или `ark_core::db` (Rust direct writers).
- **Прямые SQL writes в ARK** из app services — **запрещены**.
- **Тесты** — только на изолированных БД.
- **Substantial-правки** — через `.agent/tasks/<DATE>-<slug>/` proof loop.

## Что должно сработать прежде, чем ты начнёшь редактировать код

Прочитай в указанном порядке:

1. **[Архитектура](/concepts/architecture)** — общая картина.
2. **[Модель данных ARK](/concepts/ark-objects)** — что за таблицы и типы.
3. **[Граница записи в ARK](/concepts/write-boundary)** — что можно, что нельзя.
4. **[Изоляция тестовых БД](/concepts/test-isolation)** — как писать тесты.
5. **[Proof loop](/concepts/proof-loop)** — как оформлять substantial-задачи.
6. **[Запреты и гварды](/agents/forbidden)** — список «никогда».
7. **[Чек-листы по областям](/agents/checklists)** — что прогнать перед сдачей.

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

## Карта приложений и пакетов

Когда пользователь упоминает имя — ты должен моментально знать, где это.

| Имя | Где | Что |
|---|---|---|
| **Eden** | `apps/eden/ts` | заметки (Vue + Electron + Heart Rust) |
| **Delphi** | `apps/delphi/ts` | задачи (Electron) |
| **Arrancador** | `apps/arrancador` | игровая библиотека (Electron) |
| **Dashboard** | `apps/dashboard` | read-only аналитика (Electron) |
| **Horologion** | `apps/horologion` | трекер времени, pomodoro (WIP). `time_entry_obj` + общий `tag_obj` |
| **Digital Cave** | `apps/digital-cave` | focus-блокер (TBD, имя зарезервировано) |
| **ark-service** | `apps/ark-service` | Android Room ContentProvider для `apps/delphi/kotlin` (отдельно от desktop ARK) |
| **ark-core** | `packages/ark-core/rust` | Rust runtime + ark-core-rpc |
| **@kepler/ark** | `packages/kepler-ark` | TS SDK |
| **ark-relay-server** | `services/ark-relay-server` | WebSocket relay (опционально, для NAT) |
| **kepler-visuals** | `packages/kepler-visuals` | UI токены, тема, компоненты |
| **usage-tracker** | `services/usage-tracker` | Rust фон-сервис, пишет usage в ARK |

## Что считается substantial (нужен proof loop)

- Новая фича приложения.
- Новый ARK endpoint в `ark-core-rpc` или метод в `@kepler/ark`.
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

Полный справочник правил — [Правила репозитория](/reference/rules).
