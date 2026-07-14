# Eden: базовая страница «Всё»

## Классификация

`FULL_LOOP`: новая домашняя страница Eden затрагивает системный typed object, store-навигацию, UI, тесты и source-документацию.

## Цель

Сделать существующее состояние Eden `activeScreen === "notes" && currentEntry === null` домашней страницей «Всё». Страница показывает единую смешанную ленту уже загруженных summaries заметок и ARK-книг, а каждый новый запуск Eden начинается с этой страницы.

## В scope

- встроенный тип `book_obj` с названием в `Entry.title`, полями `author` и `cover_image` в `header_props_json`, а также Markdown-телом;
- домашняя страница «Всё» без нового route или значения `activeScreen`;
- пункт «Всё» в sidebar, возврат домой при закрытии записи и скрытый reader/writer toggle на домашней странице;
- карточки заметок и книг, CSS multi-column layout и fallback обложки;
- unit, browser component и headless Electron visual coverage;
- обновление `docs-site/apps/eden/ui.md` и `docs-site/apps/eden/typed-notes.md` с синхронизацией generated agent docs.

## Вне scope

- отдельное приложение Singularity, переименование Eden, интеграция Akasha или EPUB;
- новый ARK/RPC/data API, изменение sync-протокола, SQLite schema или extension manifest;
- file picker, прогресс чтения, ISBN, рейтинг, универсальные карточки остальных типов и фильтрующие chips;
- release/version bump;
- любые изменения в уже модифицированном пользователем `platform/desktop/package.json`.

## Acceptance Criteria

**AC1. Домашнее состояние и навигация.** Каждый новый запуск Eden начинается в `activeScreen === "notes"`, без выбранной записи и коллекции, независимо от старых ключей Diary/last-entry. `openEverything()` отменяет pending-навигацию, очищает запись и выбранный тип, sidebar первым показывает активируемое «Всё», закрытие записи возвращает домой, reader/writer toggle дома скрыт. Внутрисессионные Back/Forward продолжают возвращать из редактора на «Всё».

**AC2. Лента Everything.** `EverythingView` получает `entries` и `noteTypes`, использует только уже загруженные `eden.entries`, показывает только `note_obj` и `book_obj`, уважает существующий visible-type filter, сортирует по `updated_at DESC`, показывает русское empty state и эмитит `open-entry` с id. Task, journal, collection и прочие типы в ленту не попадают; новый data API и hydration Markdown body не добавляются.

**AC3. Системный тип книги.** `book_obj` зарегистрирован через существующие system-type/ARK write paths. Название хранится в `Entry.title`; optional `author: string` и `cover_image: string` находятся в `header_props_json`; `cover_image` имеет field kind `image` и является `imageFieldId`; Markdown body доступен для заметок о книге. Коллекция называется «Книги», использует существующую иконку `book` и текущий accent fallback. SQLite migration, новый RPC и direct SQL отсутствуют.

**AC4. Карточки и masonry-поведение.** `EverythingItemCard` имеет типизированные props/emits и отображает книгу с естественными пропорциями обложки, названием и автором либо типографической заглушкой при отсутствии/ошибке изображения; заметка показывает название, тип и дату. Карточка доступна с клавиатуры и открывает существующий `eden.navigateTo(id)`. CSS multi-column автоматически выбирает число колонок, использует один и тот же token для `column-gap` и нижнего отступа карточек, задаёт `break-inside: avoid`, переходит в одну колонку на узкой ширине и использует только Eden/Visuals tokens.

**AC5. Контракты и документация.** `App.vue` остаётся композиционным слоем, компоненты общаются через типизированные props/emits, внешние ARK/sync/schema/manifest contracts не меняются. Source-документация Eden UI и typed objects описывает домашнюю страницу и `book_obj`; generated docs синхронизированы.

**AC6. Доказательство.** Unit/store tests покрывают system type, всегда-домашний startup и `openEverything()`. Browser component tests покрывают mixed fixture, сортировку/фильтрацию, обе book cover ветки, note, empty state, emit и computed layout styles. Headless Electron test создаёт note/books через обычный ARK API с deterministic data-URI covers, проверяет cold reopen summaries, baseline `eden-everything-mixed`, открытие книги и Back на «Всё». Все заявленные финальные команды проходят, visual baseline обновлён и затем повторно проходит без update-флага; независимый verifier подтверждает каждый AC по текущему worktree.

## Проверки

```powershell
rtk bun run --cwd products/eden test:unit
rtk bun run --cwd products/eden test:vue
rtk bun run ark:guard:writes
rtk bun run --cwd platform/desktop build:js
rtk bun run docs:sync
rtk bun run docs:check
rtk bun run visual:eden -- --update-snapshots
rtk bun run visual:eden
```
