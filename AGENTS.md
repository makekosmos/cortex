# AGENTS.md — Kosmos

Техническая документация живёт в приватном репозитории
[`makekosmos/docs`](https://github.com/makekosmos/docs). Этот файл — компактный
контекст для работы с `core`; подробности открывай по ссылкам только по необходимости.

---

# Core context для Claude Code

::: tip Что это
Источник корневых `AGENTS.md` / `CLAUDE.md`. Держи этот файл коротким: он грузится в каждую сессию. Детали — только по pointer'ам ниже.
:::

## За 30 секунд

- **Kosmos** = Windows-only desktop product + монорепо (Bun workspaces + Cargo workspace).
- **Kepler** = legacy/internal namespace (`kepler:*`, `window.kepler`, `platform/runtime/`), не user-facing brand.
- **ARK** = общий Rust+SQLite рантайм (`core/ark/crates/ark-core`, `ark-core-rpc`).
- **Extensions** (`products/<id>/` / legacy `extensions/<id>/`) — Vue-приложения внутри shell.
- Apps говорят с ARK только через `@kosmos/ark` / Eden shim / `ark_core::db`; direct SQL writes в sync-таблицы запрещены.
- Перед правкой классифицируй: `NO_LOOP`, `LIGHT_LOOP`, `FULL_LOOP`.

## Execution discipline

Контекст бери в порядке доказательства, а не по области «на всякий случай»:

1. Сформулируй конкретный вопрос к коду.
2. `rtk grep` / `rg` по символу, CSS class, IPC/RPC channel, operation id, тексту ошибки.
3. Читай только найденный snippet вокруг места.
4. Открывай docs/skills только если snippet не отвечает на вопрос или задача затрагивает boundary.

Для `NO_LOOP` / `LIGHT_LOOP` не открывай broad docs/skills заранее. Если простая задача внезапно требует архитектурных docs или нескольких подсистем — остановись и эскалируй в `FULL_LOOP`.

Полный протокол: `https://github.com/makekosmos/docs/blob/main/agents/execution-protocol.md`.

## Routing: когда docs действительно нужны

| Триггер                                                  | Читать                                                                                                                                   |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| ARK/data/sync/schema/write boundary                      | `https://github.com/makekosmos/docs/blob/main/concepts/write-boundary.md`, `ark-objects.md`, `sync.md`, `agents/forbidden/ark.md`        |
| Eden layout/titlebar/sidebar visual rules                | `https://github.com/makekosmos/docs/blob/main/apps/eden/ui.md`; forbidden UI/Eden только если меняешь соответствующий запретный boundary |
| Eden editor/scroll/CM/TipTap internals                   | `https://github.com/makekosmos/docs/blob/main/apps/eden/editor.md`                                                                       |
| Eden typed objects/person/game/image model               | `https://github.com/makekosmos/docs/blob/main/apps/eden/typed-notes.md`                                                                  |
| Eden CRUD/import/search/data behavior                    | `https://github.com/makekosmos/docs/blob/main/apps/eden/data.md`, `concepts/write-boundary.md`                                           |
| Focus mode/helper/svc                                    | `https://github.com/makekosmos/docs/blob/main/concepts/focus-mode.md`, `agents/forbidden/focus-mode.md`                                  |
| Shell architecture: loader/dev mode/command bus/security | `extension-host.md`, `extension-dev-mode.md`, `command-bus.md`, `agents/forbidden/shell.md`                                              |
| Shell local IPC/window controls                          | сначала grep snippets в `platform/desktop/electron/*preload*` и `extension-host.ts`; docs не нужны, если не меняешь архитектуру loader'а |
| Tests/e2e/headless                                       | `https://github.com/makekosmos/docs/blob/main/agents/testing.md`, `agents/forbidden/tests.md`                                            |
| Release/bump/distribution                                | skill `bump`, `agents/forbidden/distribution.md`                                                                                         |
| Substantial фича/архитектура                             | `https://github.com/makekosmos/docs/blob/main/concepts/proof-loop.md` + `.agent/tasks/<DATE>-<slug>/spec.md`                             |
| Оценка срока                                             | skill `estimate-calibration`                                                                                                             |

## Команды

```powershell
bun run ark:guard:writes
bun run ark:smoke
bunx playwright test --config platform/desktop/playwright.config.ts  # headless only
```

Noisy build output — через `rtk err <cmd>` или лог в `.tmp/*.log`, в чат только ошибки/хвост.

## Карта

| Имя               | Где                                                                            |
| ----------------- | ------------------------------------------------------------------------------ |
| Eden              | `products/eden/`                                                               |
| Delphi            | `products/delphi/`                                                             |
| Focus mode        | `platform/desktop/electron/focus-*`, `platform/native-services/kepler-focus-*` |
| Arrancador        | `incubator/arrancador/`                                                        |
| Desktop shell     | `platform/desktop/`                                                            |
| Runtime/backend   | `platform/runtime/`                                                            |
| ARK core          | `core/ark/crates/ark-core/`                                                    |
| `@kosmos/ark`     | `core/ark/packages/ark/`                                                       |
| `@kosmos/visuals` | `packages/visuals/`                                                            |
| Dashboard         | `platform/desktop/src/views/Dashboard*.vue`                                    |

## Universal never rules

- **Принцип нулевого техдолга:** не добавляй временные aliases, compatibility-ветки или legacy-имена без согласованного срока удаления и проверки, которая не позволит им стать постоянными.
- ❌ Direct SQL writes в ARK sync tables из app TS / renderer; renderer не открывает SQLite.
- ❌ Rust writer в sync table без version-vector bump (`record_local_upsert/delete` или equivalent).
- ❌ Destructive migrations (`DROP TABLE`, несовместимый `ALTER COLUMN`). Только additive.
- ❌ `Mutex::lock().unwrap()` в production Rust paths; нужен poison recovery.
- ❌ E2E без `KOSMOS_HEADLESS=1`; BrowserWindow show/focus/always-on-top без headless guard.
- ❌ `backgroundMaterial` / `setBackgroundMaterial()` на transparent Win32 overlay даже со значением `"none"`; reused topmost overlay без повторного `screen-saver` level перед `showInactive()`.
- ❌ User-facing UI на английском; hardcoded `#hex`/`rgb()`/fonts вместо `@kosmos/visuals` tokens.
- ❌ Свой titlebar/safe-area вместо `<DesktopChrome>` / `<DesktopContentSurface>`.
- ❌ `e.key === "<латинская буква>"` для Ctrl/Cmd shortcuts; используй `e.code === "KeyA"`.
- ❌ `addEventListener` без cleanup в `onBeforeUnmount`.
- ❌ `git add -A`, `--no-verify`, `git reset --hard`, force-push в main/master.
- ❌ Bump/release/version change без явной команды пользователя.
- ❌ Попутный рефакторинг; один логический change — один коммит.
- ❌ Создавать или сохранять legacy product/app IDs для удобства миграции. Если ID необходимо сменить, выполняй полную миграцию всех потребителей и удаляй старый ID без fallback.
- ❌ Объявлять PASS без релевантных checks; для UI — visual verify или явно “не проверял визуально”.

Полный список: `https://github.com/makekosmos/docs/blob/main/agents/forbidden/index.md` и узкие файлы в `https://github.com/makekosmos/docs/blob/main/agents/forbidden/`.

## Классификация задач

- `NO_LOOP` — точечная строка/опечатка/локальная косметика. Просто правь и проверь релевантно.
- `LIGHT_LOOP` — маленькая low-risk правка в одном view/extension/script. Финал: классификация, проверки, что не проверено; UI — visual verify если возможно.
- `FULL_LOOP` — новая фича, endpoint, schema/sync/write-boundary/focus/security/command-bus, несколько подсистем или сомнение. Нужен proof loop.

`LIGHT_LOOP` эскалируется в `FULL_LOOP`, если затронуты ARK/data/sync/focus/command bus/schema/security или проверка локально невозможна.

## Документация

Документация поддерживается в [`makekosmos/docs`](https://github.com/makekosmos/docs).
Pointer'ы: `agents/index.md`, `execution-protocol.md`, `checklists.md`, `reference/commands.md`, `reference/smoke-matrix.md`.
