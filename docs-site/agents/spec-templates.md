# Шаблоны спецификаций

Типовые `spec.md` для proof-loop задач. Скопируй подходящий, замени плейсхолдеры.

## Базовый шаблон

```markdown
# <YYYY-MM-DD> <slug>

## Context

<В двух предложениях — зачем эта задача и что было до неё.>

## Scope

В задаче:

- <пункт>
- <пункт>

Не в задаче:

- <пункт>
- <пункт>

## Acceptance Criteria

AC1. <Проверяемое утверждение, которое верифицируется конкретной командой.>

AC2. <…>

AC3. <…>

## Verification commands

- `bun run ...` — проверяет AC1.
- `cargo test ...` — проверяет AC2.
- Manual: открыть приложение и убедиться что <…> — AC3.

## Out of scope decisions

<Если по дороге обнаружились вопросы, не входящие в задачу — фиксируй их здесь.>
```

## Шаблон: новая фича Electron-приложения

```markdown
# 2026-MM-DD <app>-<feature>

## Context

<Приложение> сейчас не умеет <X>. Пользователь хочет <Y>. Это будет интегрировано через <ARK endpoint / preload API / UI компонент>.

## Scope

В задаче:

- Добавить preload-метод `<appName>Api.<methodName>`.
- Добавить UI-компонент в `apps/<name>/src/...`.
- Подключить через store.
- Добавить unit-тесты на store и интеграционный e2e.

Не в задаче:

- Покрытие смежных приложений.
- Изменения в ARK runtime / `@kosmos/ark` API.

## Acceptance Criteria

AC1. `bun run --cwd apps/<name> typecheck` — зелёный.
AC2. `bun run --cwd apps/<name> test` — зелёный, добавлены тесты на новый store.
AC3. `bun run --cwd apps/<name> test:e2e` — зелёный, добавлен e2e сценарий <…>.
AC4. `bun run --cwd apps/<name> build` — собирается.
AC5. `bun run ark:guard:writes` — зелёный (никаких прямых SQL writes не добавлено).

## Verification commands

См. AC.
```

## Шаблон: новый ARK runtime endpoint

```markdown
# 2026-MM-DD ark-<endpoint-slug>

## Context

В app-коде сейчас приходится делать <N> отдельных RPC вызовов для <X>. Добавляем агрегированный endpoint в `ark-core-rpc`, чтобы приложения могли запросить готовый результат.

## Scope

В задаче:

- `core/ark/crates/ark-core/rust/src/db.rs` — функция-агрегатор.
- `core/ark/crates/ark-core/rust/src/main.rs` — регистрация RPC operation `<snake_case_name>`.
- `core/ark/crates/ark-core/rust/src/types.rs` — request / response типы.
- Rust unit test для агрегатора.
- `core/ark/packages/ark/src/ark-client.ts` — обёртка `ark.<group>.<methodName>`.
- TS тип для request / response.

Не в задаче:

- Интеграция в конкретное приложение (отдельная задача).
- Изменение существующих endpoints.

## Acceptance Criteria

AC1. `cargo test --manifest-path crates\ark-core\rust\Cargo.toml` — зелёный, новый тест есть.
AC2. `cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc` — собирается.
AC3. `bun run --cwd core/ark/packages/ark typecheck` — зелёный, новый метод типизирован.
AC4. `bun run --cwd core/ark/packages/ark build` — собирается.
AC5. Wire-формат остался `snake_case`. Self-peer / routable filtering не тронуты.
AC6. Schema-добавления (если были) — additive.

## Verification commands

См. AC.
```

## Шаблон: миграция данных

```markdown
# 2026-MM-DD migrate-<source>-to-<target>

## Context

Сейчас данные <X> лежат в <legacy>. Решено мигрировать в <new> (см. ADR в `docs/<...>.md`). Эта задача — реализация миграции.

## Scope

В задаче:

- Скрипт миграции `apps/<name>/.../<file>.ts`.
- Хук миграции на старте приложения.
- Идемпотентность (миграция запущенная второй раз — no-op).
- Unit-тест на миграцию с isolated DB.
- Документация в `docs/<DECISION>.md` (если ещё нет).

Не в задаче:

- Удаление legacy таблиц (это после стабилизации, отдельная задача).

## Acceptance Criteria

AC1. На пустой ARK DB — миграция no-op, тест проходит.
AC2. На DB с legacy данными — после миграции <N> записей в `<target>`, и они проходят round-trip через ARK API.
AC3. Повторный запуск миграции — no-op (идемпотентность).
AC4. `bun run --cwd apps/<name> test` — зелёный.
AC5. `bun run ark:guard:writes` — зелёный (миграция использует ARK API, не raw SQL).
AC6. Тест использует изолированную DB в `.agent/tasks/<TASK>/smoke/`.

## Verification commands

См. AC.
```

## Шаблон: багфикс

```markdown
# 2026-MM-DD fix-<short-slug>

## Context

В приложении <X> при <условие> происходит <неправильное поведение>. Ожидалось <правильное поведение>. Воспроизводится: <шаги>.

Корневая причина: <гипотеза>.

## Scope

В задаче:

- Исправить корневую причину минимальным diff'ом.
- Добавить regression-тест.

Не в задаче:

- Рефакторинг смежного кода.
- Похожие баги в других местах (если найдены — отдельная задача).

## Acceptance Criteria

AC1. Regression-тест воспроизводит баг **до** фикса (red).
AC2. После фикса regression-тест зелёный.
AC3. `bun run --cwd <app> test` — весь набор зелёный, ничего не сломалось.
AC4. `bun run --cwd <app> build` — собирается.

## Verification commands

См. AC.
```

## Подсказки по AC

- AC формулируется как **проверяемое утверждение**, не «улучшить».
- Каждый AC должен ссылаться на конкретную команду или конкретный артефакт.
- Минимум 2 AC. Если получился один — задача слишком мелкая для proof loop.
- AC не редактируется после старта реализации. Если по дороге обнаружилось, что AC некорректен — это либо новая задача, либо запись в `problems.md`.
