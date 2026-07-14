# Eden: верхняя навигация без сайдбара

## Класс задачи

`FULL_LOOP`: новый пользовательский navigation workflow затрагивает корневую композицию Eden, layout-state, горячие клавиши, browser/Electron проверки и source-документацию.

## Цель

Убрать боковую панель Eden и заменить её постоянным верхним переключателем из двух разделов: «Всё» и «Дневник».

## В scope

- Верхняя навигация в обычном режиме Eden с двумя и только двумя пользовательскими пунктами: «Всё» и «Дневник».
- Переходы через существующие `openEverything()` и `openDiary()` без новых route или screen-state.
- Удаление sidebar UI и Eden-local состояния, persistence и shortcut, которые обслуживали только sidebar.
- Сохранение существующих Back/Forward, editor, focus mode, search и diary calendar.
- Обновление source docs, browser/unit coverage и Electron visual baseline.

## Вне scope

- Удаление существующего внутреннего `type-collection` screen или ARK typed-object model.
- Изменение ARK API, SQLite, sync, extension manifest или desktop sidebar IPC compatibility layer.
- Возврат settings/object collections в новую навигацию.
- Release, bump, commit или изменение `platform/desktop/package.json`.

## Acceptance Criteria

**AC1.** В обычном режиме Eden не рендерит sidebar, кнопку его раскрытия или sidebar context menu; неиспользуемые sidebar-компоненты и их browser test удалены.

**AC2.** Сверху над основным содержимым отображается доступная навигация ровно с двумя кнопками «Всё» и «Дневник». Активный раздел имеет программно определимое и визуальное состояние; в focus mode навигация скрыта.

**AC3.** «Всё» вызывает существующий `eden.openEverything()`, «Дневник» — `eden.openDiary()`. Переходы работают с домашней страницы, из редактора и между разделами без нового route или `activeScreen`.

**AC4.** Eden-local sidebar width/hidden state, persistence initialization и Ctrl/Cmd+B shortcut удалены; оставшиеся search, zen, Back/Forward и diary calendar сценарии продолжают работать.

**AC5.** UI использует только существующие Eden/Visuals CSS tokens, не пересекается с основным содержимым на проверенном desktop viewport и задокументирован в `docs-site/apps/eden/ui.md`.

**AC6.** Релевантные Eden unit/browser tests, ARK write guard, desktop JS build, docs sync/check и headless Electron visual test проходят на текущем worktree. Новый visual baseline подтверждает отсутствие sidebar и работу верхнего переключателя.
