# Eden: cache lifecycle дневника

## Класс задачи

`FULL_LOOP`: исправление меняет lifecycle data-backed view и должно доказать отсутствие повторных ARK-read при навигации.

## Цель

После первой загрузки дневник должен сохранять отрисованные данные между переходами «Всё» ↔ «Дневник» и получать обновления через уже существующую ARK-подписку, без повторного холодного запуска при каждом открытии.

## В scope

- Сохранение instance `BubbleDiaryView` после первого открытия через нативный Vue cache lifecycle.
- Ленивая первая загрузка: дневник не монтируется до первого перехода на него.
- Существующие ARK migrations/read и live subscription остаются внутри `BubbleDiaryView`.
- Один browser regression test на повторное открытие и live-update во время неактивного состояния.
- Обновление source-документации Eden data lifecycle.

## Вне scope

- Новый Pinia/cache store, TTL, RPC, SQLite schema или sync protocol.
- Изменение bubble CRUD/write-path, миграций или формата данных.
- Persistence UI-cache между перезапусками Eden.
- Release, commit или изменение `platform/desktop/package.json`.

## Acceptance Criteria

**AC1.** Первое открытие дневника запускает существующий `startDiary()` один раз; переход на «Всё» и повторное открытие возвращают тот же cached component instance без нового `list_objects_by_type` read.

**AC2.** Пока дневник неактивен, существующая ARK subscription продолжает обновлять его локальную timeline; при возврате новое событие видно без дополнительного navigation-triggered read.

**AC3.** Первый mount остаётся ленивым, cache ограничен одним instance, а окончательный unmount Eden по-прежнему выполняет существующий cleanup listeners/subscriptions/timer.

**AC4.** Не добавлены новый cache abstraction, dependency, ARK API, write-path или schema change. `ark:guard:writes` проходит.

**AC5.** Eden browser suite, unit suite, targeted extension build, docs sync/check и свежий verifier проходят; source docs описывают cached/live-refresh lifecycle дневника.
