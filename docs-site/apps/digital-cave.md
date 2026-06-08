# Digital Cave — фокус-блокер

::: warning Статус: частично реализовано
Полноценное приложение `apps/digital-cave/` **не создано**. Часть функциональности уже работает как [focus-mode подсистема](/concepts/focus-mode) внутри Kepler shell — floating focus widget + domain blocking через hosts file. Полноценный Digital Cave (app-level blocks, hard mode, schedules, zen UI deep-focus режим) — пока не реализован отдельно.
:::

- **Path**: `apps/digital-cave` (зарезервировано)
- **Реализованная часть**: `platform/desktop/electron/focus-*.ts` + `platform/native-services/kepler-focus-helper/` + `platform/native-services/kepler-focus-svc/` + `platform/desktop/src/views/FocusWidgetView.vue` + `platform/runtime/src/focus.rs`. См. [focus-mode](/concepts/focus-mode).
- **Аналог**: [Cold Turkey Blocker](https://getcoldturkey.com/) — блокировщик отвлекающих сайтов / приложений на запланированный фокус-период.

## Что уже работает (focus-mode)

- Floating always-on-top widget с countdown'ом фокус-сессии (Spotify-mini-style).
- Блокировка доменов через `C:\Windows\System32\drivers\etc\hosts` (managed-секция между маркерами).
- Zero-UAC режим после первой установки Windows-сервиса `kepler-focus-svc`.
- Хранение blocklist'ов как `blocklist_obj` объектов в ARK.

Подробности — [Focus mode](/concepts/focus-mode).

## Что ещё не реализовано (Digital Cave proper)

- Чёрный список **приложений** (по `process_name` / window title) — focus-mode умеет только домены.
- Расписание (например: «будни 9–13 — блок соц.сетей»).
- Hard mode: блок нельзя выключить до конца таймера.
- Логирование попыток обхода (`block_attempt_obj` в ARK).
- Zen UI deep-focus режим (отдельное окно, отключение нотификаций, и т.п.).

## Что НЕ делать

- ❌ Системные блокировки на уровне hosts-файла без user-consent. (Текущий focus-mode пишет в hosts только когда юзер активировал blocklist через UI.)
- ❌ Шпионаж (логирование того что юзер набирает / сайтов кроме блокированных).

## Когда

## Связанные документы

- [Focus mode](/concepts/focus-mode) — текущая реализация (widget + domain blocking).
- [usage-tracker](/services/usage-tracker) — источник данных о процессах (для будущей app-level блокировки).
