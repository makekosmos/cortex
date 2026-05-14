# Рабочий процесс

## Когда нужен proof loop

Если задача — это **substantial feature**, **рефакторинг** или **нетривиальный багфикс**, она обязательно проходит через [proof loop](/concepts/proof-loop): создаётся `.agent/tasks/<DATE>-<slug>/spec.md`, реализация идёт против явных AC, результат подтверждается `evidence.md` / `evidence.json`.

Для тривиальных правок (опечатки, edit-level, переименование переменной, перенос строки в README) proof loop **не нужен**.

## Стандартный цикл правок

```text
1. Понять задачу     →    обсудить с человеком / создать spec
2. Замёрзить spec    →    .agent/tasks/<DATE>-<slug>/spec.md с AC1..ACn
3. Реализовать       →    наименьший defensible diff
4. Прогнать гварды   →    bun run ark:guard:writes (если затронут data path)
5. Прогнать smoke    →    bun run ark:smoke
6. Зафиксировать     →    evidence.md + evidence.json + raw/
7. Verify (свежий)   →    повторить команды против текущего кода
8. Если не PASS      →    problems.md → fix → reverify
9. Коммит            →    осмысленное сообщение, не «add big»
```

## Правила коммитов

- Сообщение в активном залоге, кратко описывающее **почему**, не **что**.
- Не коммить файлы с секретами (`.env`, `credentials.json`).
- Один логический change — один коммит. Не сваливай миграцию и UI-правки в один коммит.
- Не амендь опубликованные коммиты, лучше создай новый.

## Когда нельзя

- ❌ Прямой SQL `INSERT`/`UPDATE`/`DELETE` в таблицы ARK из приложения. См. [Граница записи](/concepts/write-boundary).
- ❌ Дефолтный путь к user ARK DB в тестах. См. [Изоляция тестовых БД](/concepts/test-isolation).
- ❌ Возврат старого Delphi DB sidecar.
- ❌ Возврат собственного usage tracker внутри Arrancador (он живёт в `services/kepler-backend/src/usage_tracker`).
- ❌ Возврат ripgrep как поискового движка Eden — он на Tantivy через Eden Heart.
- ❌ Дублирование UI-компонентов, которые уже есть в `@kepler/visuals` (Sidebar, Titlebar, DesktopChrome).
- ❌ `--no-verify` при коммите.

## Когда нужно

- ✅ Перед PR в data-слой — `bun run ark:guard:writes`.
- ✅ Перед PR в любую часть ARK — `cargo test` + `bun run --cwd packages/kosmos-ark typecheck`.
- ✅ Перед PR в Electron-приложение — `bun run typecheck`, `bun run build`, `bun run test:e2e`.
- ✅ Все новые тестовые БД — изолированные. Передавай путь через CLI/env, не дефолти в user data.
- ✅ Если меняешь endpoint в `ark-core-rpc` — добавь тест миграции и репликации, не только локальный CRUD.

## Стиль кода

- Чистый, читаемый код > «умный» код.
- Не добавляй комментарии, объясняющие **что**. Понятные имена делают это сами.
- Комментарий уместен только когда есть скрытое ограничение или неочевидный инвариант.
- Не пиши документацию задним числом — обнови `docs-site/` сразу, если меняешь концепт.
- Никаких `console.log` / `dbg!` / закоментированного мусора в коммите.
