# Akasha

Akasha — EPUB-читалка для Kosmos.

## Статус

MVP: обычный `kind: "vue"` extension внутри Kosmos shell. Запускается
launcher-командой `akasha:open`, рендерится в стандартном extension
`BrowserWindow` и собирается тем же Vite pipeline, что Eden, Delphi,

Старый Rust/GPUI reader вынесен в приватный standalone-репозиторий
`ksanrse/akasha-gpui` и удалён из локального Kosmos workspace.

Akasha теперь стартует с локальной библиотеки: пользователь добавляет `.epub`,
файл импортируется в `extensions-data/akasha/books/<bookId>.epub`, а карточка
книги появляется в UI с прогрессом и действием «Продолжить». `bookId` считается
как SHA-256 содержимого EPUB, поэтому модель готова к дедупликации повторного
импорта.

Reader UI — один непрерывный поток EPUB spine вниз: главы идут друг за другом
без ручного переключения. Слева показывается оглавление, toolbar управляет
шрифтом/размером текста и может добавить новую книгу. XHTML-структура
сохраняется в безопасную internal-модель: заголовки, абзацы, списки, цитаты,
bold и italic рендерятся Vue-компонентами без `v-html`.

## Где код

| Часть             | Путь                                            | Роль                                              |
| ----------------- | ----------------------------------------------- | ------------------------------------------------- |
| Manifest          | `incubator/akasha/manifest.json`                | Команда launcher'а + Vue entrypoint               |
| Vue app           | `incubator/akasha/src`                          | EPUB reader UI + parser                           |
| User data         | `extensions-data/akasha`                        | Библиотека, EPUB-копии, progress, reader settings |
| Build integration | `platform/desktop/scripts/build-extensions.mjs` | Vite build через shared extension config          |

## Контракт

`incubator/akasha/manifest.json` объявляет:

- `kind: "vue"`;
- `entryHtml: "dist/index.html"`;
- `devPort: 5185`;
- manifest-команду `akasha:open`.

Akasha использует стандартный extension preload. Reader-local настройки
пишутся через `window.kepler.userData.writeJson("reader-state.json", ...)`, а
EPUB-копии — через binary userData API (`readBinary` / `writeBinary`) с safe
relative paths внутри namespace extension'а. Для dev/browser-сценариев каталог
имеет fallback в `localStorage`.

## EPUB parser

Akasha читает `.epub` в renderer'е как ZIP:

1. Находит `META-INF/container.xml`.
2. Читает OPF package и spine order.
3. Распаковывает XHTML-файлы из spine.
4. Превращает XHTML в `ReaderBlock[]` + `InlineSpan[]`.

В UI не используется `v-html`: EPUB-текст выводится как escaped Vue text nodes,
а emphasis/strong представлены классами на spans.

## Данные

Akasha v1 не пишет в ARK. Локально сохраняются:

- `library.json` — catalog imported books;
- `books/<bookId>.epub` — app-local EPUB copy;
- `reader-state.json` — reader settings;
- progress внутри `library.json`: `chapterId`, `blockId`, `percentage`,
  `updatedAt`.

Позиция чтения намеренно сохраняется не как `scrollTop`, а как EPUB block id:
это устойчивее к изменению шрифта, размера окна и line-height. ARK-backed
highlights, notes, semantic/RAG features — отдельная будущая фаза.

## Проверки

```powershell
bun run --cwd platform/desktop build:extensions
bun run --cwd platform/desktop typecheck
```
