# Поддержка документации в актуальном состоянии

::: danger Ты — агент, читающий это
Эта страница — **обязательный** контекст. Документация Kosmos сейчас — единственный источник правды для людей и агентов. Если она устарела, ты как агент примешь неверные решения и сломаешь ожидания пользователя. Перед закрытием **любой** substantial-задачи пройди раздел [«Что проверить»](#что-проверить).
:::

## Базовый принцип

```
Изменил поведение / API / структуру → обновил docs-site/ → bun run docs:sync → bun run docs:check
```

Документация **обязана** меняться в **той же** задаче, что и код. Не «потом», не «когда руки дойдут». Иначе drift начинается в день один.

## Когда обновлять документацию

| Что изменил                                    | Где обновить                                                                                  |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Добавил/убрал команду в `package.json`         | `docs-site/reference/commands.md` + соответствующее место в `docs-site/apps/<name>.md`        |
| Изменил ARK schema / endpoint в `ark-core-rpc` | `docs-site/concepts/ark-objects.md` + `docs-site/packages/ark-core.md`                        |
| Добавил/изменил метод в `@kosmos/ark`          | `docs-site/packages/ark.md` + примеры в `docs-site/concepts/ark-objects.md`                   |
| Изменил структуру папок приложения             | `docs-site/apps/<name>.md` и `docs-site/guide/layout.md`                                      |
| Удалил/перенёс файл, упомянутый в доке         | grep по `docs-site/` на имя файла → обновить или удалить упоминание                           |
| Изменил sync-протокол / HLC / relay            | `docs-site/concepts/sync.md`                                                                  |
| Добавил/убрал зависимость в стеке              | `docs-site/guide/tooling.md`                                                                  |
| Добавил smoke-команду                          | `docs-site/reference/smoke-matrix.md`                                                         |
| Изменил правило/запрет                         | `docs-site/agents/forbidden.md` или `docs-site/reference/rules.md`                            |
| Принял архитектурное решение                   | новый файл `docs/<DECISION>.md` (полный ADR) + ссылка в `docs-site/reference/decisions.md`    |
| Изменил дизайн-токены `@kosmos/visuals`        | `docs-site/packages/visuals.md` + при необходимости `docs-site/.vitepress/theme/custom.css`   |
| Создал/убрал `object_type`                     | `docs-site/concepts/ark-objects.md` (таблица «Известные типы») + соответствующая app-страница |
| Запланировал фичу / нашёл баг приложения       | `docs-site/apps/<name>-roadmap.md` (см. [Roadmap-конвенция](#roadmap))                        |

## Когда **НЕ** надо трогать документацию

- Локальное переименование private-переменной без публичного эффекта.
- Чистка `console.log`, опечатки в комментариях.
- Edit-level правка одной строки в UI без изменения поведения.
- Patch-обновление зависимости без эффекта на пользователя.

## Workflow обновления

1. **Сначала правь под `docs-site/`.** Не правь сгенерированные `AGENTS.md` / `CLAUDE.md` / `apps/*/AGENTS.md` напрямую — твои изменения будут затёрты при следующем `docs:sync`.
2. **Запусти `bun run docs:sync`.** Регенерирует все `AGENTS.md`, `CLAUDE.md`, `llms.txt`.
3. **Запусти `bun run docs:check`.** Проверяет, что упомянутые в доке пути/команды/файлы реально существуют.
4. **Коммить и доку, и код в одном PR.** Не отдельно.

## Что проверить перед закрытием задачи

Чек-лист обязательный, не пропускай:

- [ ] Я перечитал `docs-site/apps/<которые трогал>.md` — там нет устаревших фактов?
- [ ] Если добавил/убрал команду — отразил в `docs-site/reference/commands.md`?
- [ ] Если изменил публичный API (ARK endpoint, `@kosmos/ark` метод, preload) — обновил соответствующую страницу пакета/приложения?
- [ ] Если ввёл новое архитектурное решение — есть ADR в `docs/` и ссылка в `docs-site/reference/decisions.md`?
- [ ] `bun run docs:sync` прошёл без ошибок.
- [ ] `bun run docs:check` зелёный (нет stale-references).
- [ ] `bun run docs:build` собирается без warnings про невалидные ссылки.

## Команда `docs:check`

Полная аудит-команда:

```powershell
bun run docs:check
```

Что делает (`scripts/check-docs-freshness.mjs`):

- Парсит все `docs-site/**/*.md` (кроме сгенерированных).
- Извлекает упоминания путей (`apps/<x>/...`, `crates/<x>/...`, `shell/<x>/...`, `extensions/<x>/...`, `packages/<x>/...`, `services/<x>/...`, `mobile/<x>/...`, `legacy/<x>/...`, `scripts/<x>.<ext>`, `docs/<x>.md`).
- Проверяет, что эти пути существуют в репозитории.
- Извлекает упоминания команд (`bun run <name>`, `cargo <subcmd>`).
- Проверяет, что `bun run <name>` есть в каком-то `package.json` workspace'а.
- Извлекает внутренние ссылки (`/concepts/architecture` и т.п.) и проверяет, что страница существует.
- Печатает stale-references как warnings.

Если падает — открой соответствующий `.md` и обнови или удали упоминание.

## Регенерация при подозрении на drift

Если кажется, что `AGENTS.md` или `llms.txt` устарели (например, после ребейза/мерджа):

```powershell
bun run docs:sync       # перегенерация всех auto-context файлов
bun run docs:check      # верификация
git diff                # увидишь что регенерация поменяла
```

## Анти-паттерны

- ❌ Править `AGENTS.md` напрямую «потому что я знаю что мне нужно». Будет затёрто.
- ❌ Откладывать обновление доки на «потом». Drift начинается мгновенно.
- ❌ Документировать **намерения**, а не **факты**. Не пиши «в будущем будет X». Пиши «X есть» или вообще не упоминай.
- ❌ Скрывать удаление функционала. Если убрал — убери из доки.
- ❌ Дублировать концепт на несколько страниц. Концепт живёт на одной странице, остальные ссылаются.

## Что важно понять про auto-generation

Поток:

```
docs-site/**/*.md     → bun run docs:sync →    AGENTS.md / CLAUDE.md / apps/*/AGENTS.md / llms.txt
                                              (все помечены <!-- AUTO-GENERATED -->)
```

Скрипт `scripts/sync-agents-docs.mjs` берёт:

- `docs-site/agents/index.md` + `forbidden.md` + `checklists.md` + `reference/rules.md` + `concepts/proof-loop.md` → корневой `AGENTS.md` и `CLAUDE.md`.
- `docs-site/apps/delphi.md` (Kotlin-часть) → `mobile/delphi/AGENTS.md`.
- `docs-site/packages/ark-core.md` → `crates/ark-core/AGENTS.md`.
- Весь набор ключевых страниц inline → `docs-site/public/llms.txt`.

::: tip Eden / Dashboard / Horologion / Arrancador
Per-extension `AGENTS.md` не генерируются (папки `apps/<name>/` упразднены — расширения живут в `extensions/<id>/` и читают общий корневой `AGENTS.md`). Если нужны жёсткие per-extension правила — добавляй их в соответствующую страницу `docs-site/apps/<name>.md` либо в `docs-site/agents/forbidden.md` (секция per-app).
:::

Если меняешь логику генерации — правь сам `scripts/sync-agents-docs.mjs`, потом `bun run docs:sync`.

## Roadmap

У каждого приложения может быть **отдельная страница roadmap** — `docs-site/apps/<name>-roadmap.md`. Это список того, что хочется сделать в приложении / баги / TODO. Источник информации:

- Что-то обсудили с пользователем — записываешь в roadmap (не в код, не в TODO.md, не «запомнишь»).
- Нашёл баг или ограничение в процессе работы — записываешь в roadmap → `Баги / замечания`.

### Структура roadmap-страницы

Три обязательных секции:

1. **Ближайшее** — фичи, над которыми работают сейчас или планируют скоро (по приоритету ↓). Если фича в работе — пометка статуса (`WIP — scaffold`, `WIP — UI готов`).
2. **Потом** — идеи / nice-to-have. Не критично, но не теряем.
3. **Баги / замечания** — открытые проблемы.

Пример — [Horologion Roadmap](/apps/horologion-roadmap).

### Как поддерживать

- **Когда фича в roadmap начинает делаться** — оставляй её в «Ближайшее» с пометкой статуса.
- **Когда фича завершена** — убирай из roadmap и фиксируй в обзоре приложения (`docs-site/apps/<name>.md`).
- **Баг исправили** — убираем из «Баги».
- **Не дублируй с code-комментариями `// TODO`** — комментарии в коде про _локальное место_; roadmap про _направление приложения_.

### Когда roadmap-страницы нет

Создаёшь новую — добавь её в `docs-site/.vitepress/config.ts` sidebar под соответствующим приложением как «`<Name> — Roadmap`».

## Связанные документы

- [Старт работы](/agents/) — общая стартовая страница агента.
- [Чек-листы по областям](/agents/checklists) — что прогнать после правок в коде.
- [Шаблоны спецификаций](/agents/spec-templates) — proof-loop templates.
- [Команды и скрипты](/reference/commands).
