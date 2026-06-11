# Eden: CodeMirror 6 markdown-редактор, Vim mode и shared sidebar

## Цель и контекст

Eden использует TipTap (WYSIWYG, ProseMirror JSON в ARK). Пользователь хочет typing-UX уровня
zennotes (`sample/zennotes-main`): CodeMirror 6 + live preview в стиле Obsidian — редактируется
исходный markdown, синтаксис скрывается декорациями вне строки с курсором. Это фаза 1 из 3
(ядро за флагом → паритет TaskRef/wikilinks → миграция формата хранения).

Зеннотс — MIT (Adib Hanna and ZenNotes contributors); портированные файлы несут атрибуцию
в заголовке.

2026-06-11 пользователь явно расширил текущий proof-loop: кроме начатого CM6-порта нужно
довести Eden до Zennotes-like editor UX, подключить Vim motions как в Zennotes, добавить
страницу настроек со справочником motions и toggle Vim mode, а также довести Eden sidebar
до shared sidebar из `@kosmos/visuals`.

## Скоуп

**В задаче:**

- Новый компонент `products/eden/src/editor-cm/CmEditor.vue` на CodeMirror 6, монтируется из
  `App.vue` вместо `Editor.vue` при включённом флаге и безопасном контенте.
- Порт из zennotes (`packages/app-core/src/lib/`): live preview (упрощённый: скрытие
  markdown-синтаксиса вне активной строки + интерактивные чекбоксы; БЕЗ image/PDF-виджетов и
  zustand-подписок), `cm-markdown-list-indent`, `cm-ordered-list-renumber`,
  `cm-code-block-font`, `cm-code-languages`, slash-команды (без «Page» и wikilink-зависимостей).
- Конвертация на границе: при открытии PM JSON → markdown, при сохранении markdown → PM JSON
  (через headless TipTap instance с `@tiptap/markdown`). **Формат хранения в ARK не меняется**
  (тот же `note_obj.contentJson` = PM JSON, тот же `onSave` pipeline).
- Gate-модуль: заметка открывается в CM-редакторе только если её PM JSON состоит из
  allowlist-нод/марок (paragraph, heading, text, bulletList, orderedList, listItem, blockquote,
  codeBlock, hardBreak, horizontalRule; marks: bold, italic, strike, code, link). Иначе —
  прозрачный fallback на старый TipTap-редактор (TaskRef/wikilink не теряются).
- Преференс `cmEditorEnabled: boolean` (default `false`) в `usePreferences`, переключатель в
  существующем UI настроек рядом со spellcheck.
- Преференс `vimModeEnabled: boolean` (default `false`) в `usePreferences`, переключатель в
  настройках. Vim mode включается только для CM6-редактора и не ломает fallback TipTap.
- Vim интеграция через `@replit/codemirror-vim`, как в Zennotes: базовые normal/insert/visual
  motions предоставляет библиотека; Eden регистрирует только релевантные ex-команды (`:w`,
  `:q`, `:wq`, `:zen`/`:zen toggle|on|off`) без ремаппинга.
- Страница настроек «Vim» со справочником всех Eden-visible Vim actions/motions: базовые
  motions, editing, search, visual mode, window/ex commands. Motions нельзя менять, только
  включить/выключить Vim mode.
- Eden sidebar должен использовать public API `@kosmos/visuals` (`Sidebar`, `SidebarConfig`,
  `SidebarNavItem`, `SidebarProjectItem`) без legacy-only props и локальных SVG-кнопок там,
  где есть lucide-аналог.
- CSS только через `var(--*)` токены Eden/@kosmos/visuals; каретка — accent, как в zennotes
  (`caret-color`, fat/block cursor), Vim panel тоже стилизован токенами.

**Вне задачи (следующие фазы):** TaskRef/wikilink-виджеты в CM, автокомплиты wikilink/дат,
heading fold, frontmatter, смена формата хранения на markdown, удаление TipTap, таблицы/картинки,
глобальная Vim-навигация по панелям/сплитам как в полноценном Zennotes workspace.

## Acceptance Criteria

- **AC1.** `bun run --cwd products/eden test` PASS, включая новые тесты: (a) bun:test юнит-тесты
  gate-модуля (allowlist-документ → true; документ с `taskRef`/`wikilink`/неизвестной нодой →
  false); (b) vitest browser спек round-trip конвертации (PM JSON ↔ markdown для заголовков,
  списков, чекбоксов-тасклистов нет — обычные `- [ ]` остаются текстом, code block, marks);
  (c) vitest browser спек CmEditor: монтируется, ввод текста меняет doc, emit `liveCharCount`.
  Тесты написаны и прогнаны ДО реализации (зафиксированный RED-прогон в raw/).
- **AC2.** При `cmEditorEnabled=true` и allowlist-контенте `App.vue` монтирует `CmEditor`;
  при `false` или не-allowlist-контенте — старый `Editor`. Подтверждается component-тестом
  выбора ветки или юнит-тестом функции выбора.
- **AC3.** Live preview работает: markdown-маркеры (`#`, `**`, `` ` ``) скрыты на строках без
  курсора и видимы на активной строке; `- [ ]`/`- [x]` рендерятся интерактивным чекбоксом,
  клик переключает символ в доке. Подтверждается vitest browser спеком и скриншотом.
- **AC4.** Сохранение: правка в CmEditor проходит через существующий `onSave(entry)` с
  `content_json` = валидный PM JSON (round-trip через конвертер), debounce-автосейв как у
  старого редактора (300ms) + flush на blur. Подтверждается component-тестом (mock onSave).
- **AC5.** Типчек и гварды: `bun run --cwd products/eden typecheck` (или эквивалентный скрипт)
  PASS; `bun run ark:guard:writes` PASS; в новых файлах нет hex/rgb-литералов цвета
  (grep-проверка), только `var(--*)`.
- **AC6.** Visual verify: скриншот CmEditor с документом (заголовок, список, чекбокс, код)
  под `.tmp/` или в task-папке; честная пометка что проверено глазами/скриншотом.
- **AC7.** Существующие e2e Eden не затронуты: флаг по умолчанию выключен, старый редактор —
  путь по умолчанию. `bun run --cwd products/eden test` без новых файлов-фейлов вне задачи.
- **AC8.** Vim mode: при `cmEditorEnabled=true` и `vimModeEnabled=true` CM6-редактор подключает
  `@replit/codemirror-vim`; normal-mode motions (`h/j/k/l`, `i`, `Esc`, `dd`, `u`, `/`, `:`)
  работают на уровне библиотеки, `:w` вызывает текущий `onSave`, `:zen toggle|on|off` управляет
  Eden zen mode через события. При `vimModeEnabled=false` поведение обычного CM6-ввода не меняется.
- **AC9.** Настройки: в sidebar настроек есть пункт «Vim»; страница показывает toggle Vim mode
  и полный readonly-справочник Eden-visible Vim motions/actions на русском. Пользователь не может
  менять биндинги, только включить/выключить Vim mode.
- **AC10.** Sidebar: `EdenSidebar.vue` работает через актуальный public API `@kosmos/visuals`
  без несуществующих props shared `Sidebar`, с lucide-иконками для кнопок; текущие переходы
  «заметки / настройки / типы объектов / коллекции» и collapse/resize сохраняются.
