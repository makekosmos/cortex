# 2026-06-05 — Shell command surfaces + clipboard persistence

## Контекст

Встроенные команды Shell (`Буфер обмена`, `Начать фокус`) не должны открывать
отдельные окна или менять route/root вне основного launcher surface. Shell
состоит из header/input блока и content блока; command pages отображаются как
режимы внутри `LauncherView`.

Raycast reference для Clipboard History: локальное хранение, типы
text/image/link/color/file, pin entries, поиск/фильтр, copy/open/remove/clear,
retention.

## Acceptance Criteria

**AC1.** Команда `kepler:clipboard-history` открывает Clipboard внутри текущего
Kepler Shell content slot. Новое `BrowserWindow` не создается, размер/позиция
Shell не меняются.

**AC2.** Команда `kepler:focus-session` открывает Focus Session внутри текущего
Kepler Shell content slot. Header показывает back, контент содержит цель,
длительность, Delphi task и blocklist selection.

**AC3.** Встроенные Shell command pages используют одну модель навигации:
header back возвращает в список команд; input есть только там, где нужен
поиск/фильтр.

**AC4.** Clipboard history переживает рестарт процесса через persisted
instance-scoped storage, без ARK/SQLite writes из renderer.

**AC5.** Clipboard retention настраивается по сроку и размеру. Default:
30 дней и bounded disk budget. Expired/old unpinned entries удаляются
автоматически; pinned entries защищены от pruning.

**AC6.** Settings содержит отдельную страницу Clipboard с retention controls и
текущей статистикой.

**AC7.** Есть regression coverage для clipboard persistence/pruning и visual
verify screenshots для Shell Clipboard/Focus/Settings surfaces.
