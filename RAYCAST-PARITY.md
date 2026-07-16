# Raycast parity для Kosmos

Актуальность сравнения: 2026-07-15.

Цель документа — не скопировать Raycast целиком, а отделить функции, которые
реально нужны Kosmos как ежедневному launcher, от дорогой ширины продукта.
Сравнение сделано с Raycast как продуктом целиком. Часть функций Raycast пока
относится преимущественно к macOS или облачным тарифам; Raycast for Windows
всё ещё находится в beta.

## Источники Raycast

- [Root Search](https://manual.raycast.com/search-bar)
- [Action Panel](https://manual.raycast.com/action-panel)
- [File Search](https://manual.raycast.com/file-search)
- [Extensions](https://manual.raycast.com/extensions)
- [Extension UI API](https://developers.raycast.com/api-reference/user-interface)
- [Clipboard History](https://manual.raycast.com/clipboard-history)
- [Snippets](https://manual.raycast.com/snippets)
- [Quicklinks](https://manual.raycast.com/quicklinks)
- [Calculator](https://manual.raycast.com/calculator)
- [Window Management](https://manual.raycast.com/window-management)
- [System Commands](https://manual.raycast.com/system-commands)
- [Script Commands](https://manual.raycast.com/script-commands)
- [Focus](https://manual.raycast.com/focus/how-to-create-a-shortcut)
- [Dictation](https://manual.raycast.com/ai/dictation)
- [AI Chat](https://manual.raycast.com/ai/chat)
- [AI Agents](https://manual.raycast.com/ai/agents)
- [MCP](https://manual.raycast.com/ai/model-context-protocol)
- [iOS](https://manual.raycast.com/ios)
- [Teams](https://manual.raycast.com/teams/shared-features)
- [Raycast for Windows](https://www.raycast.com/windows)

## Шкала

| Обозначение | Значение                                             |
| ----------- | ---------------------------------------------------- |
| P0          | Без этого Kosmos проигрывает как ежедневный launcher |
| P1          | Нужно для серьёзных power users                      |
| P2          | Усиливает полноту продукта и удержание               |
| P3          | Стратегическая функция после стабилизации ядра       |
| P4          | Сознательно не делать сейчас                         |

Сложность — инженерный риск, а не календарный срок.

| Сложность | Значение                                                         |
| --------: | ---------------------------------------------------------------- |
|         1 | Локальная доработка одной поверхности                            |
|         2 | Небольшая самостоятельная функция                                |
|         3 | UI + backend + persistence                                       |
|         4 | Native API, hooks, безопасность или серьёзная производительность |
|         5 | Новая продуктовая подсистема                                     |
|        5+ | Отдельная платформа или многорелизная программа                  |

## P0 — ежедневное ядро launcher

| Функция                              | Что есть в Kosmos                                                                                                                                                                                              | Что требуется до Raycast-level                                                                                                              | Сложность | Основная зависимость                                                  |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | --------: | --------------------------------------------------------------------- |
| Единый fuzzy-поиск                   | Команды ранжируются простым substring score; приложения отдельно имеют prefix/word/substring + frecency                                                                                                        | Один ранкер для apps, commands, files и extensions; typo tolerance, fuzzy, aliases, keywords, frecency, стабильный порядок и reset learning |         3 | Единый каталог сущностей и usage events                               |
| Alias для каждой команды             | Пользовательских aliases нет                                                                                                                                                                                   | Несколько aliases на команду, учёт в ranking, настройка через Settings и Action Panel                                                       |         2 | Единый каталог команд                                                 |
| Глобальный hotkey для любой команды  | Есть специальные hotkeys launcher и dictation                                                                                                                                                                  | Назначение любой комбинации любой команде, обнаружение конфликтов, physical/logical keys, отключение                                        |         4 | Общий native shortcut registry                                        |
| Универсальный Action Panel           | В root launcher в основном open/favorite/hide; command host заметно богаче                                                                                                                                     | Одна модель actions для app/file/command/extension: секции, вложенные действия, shortcuts, destructive actions, deeplinks                   |         3 | Общий action descriptor                                               |
| File Search: действия и preview      | Есть сильный локальный индекс и быстрый поиск имён; launcher показывает ограниченный inline-результат                                                                                                          | Полноценный список, preview/metadata, навигация по пути, Open With, terminal, copy path, duplicate, trash, recent files                     |         3 | Текущий file index                                                    |
| Clipboard History                    | Код существует, но функция выключена и заморожена                                                                                                                                                              | Event-driven watcher; text/images/files/colors; search, pin, edit, paste, exclusions; позднее OCR/QR и sequential paste                     |         4 | Win32 `WM_CLIPBOARDUPDATE`, не polling                                |
| Quicklinks                           | Отдельного пользовательского типа нет                                                                                                                                                                          | URL/file/deeplink, аргументы и placeholders, теги, aliases/hotkeys, выбор браузера, import/export                                           |         3 | Каталог команд и parameter schema                                     |
| Calculator и conversions             | Есть inline-калькулятор на `fend-core`: арифметика, проценты, единицы, системы счисления, даты, валюты по ежедневным курсам ЦБ РФ и базовая форма `900 долларов в рублях`; результат копируется по Enter/клику | Raycast дополнительно даёт timezone, color conversion, историю и более широкий natural language                                             |         2 | Для следующего уровня нужны timezone и расширенный русский normalizer |
| Snippets и text expansion            | Нет                                                                                                                                                                                                            | Поиск и вставка, keyword expansion, placeholders, clipboard/date/cursor variables, теги, exclusions, import/export                          |         4 | Native keyboard hook и injection                                      |
| Непрерывная keyboard-first навигация | Частично есть в launcher и command host                                                                                                                                                                        | Одинаковые shortcuts, back/navigation, empty/error/loading states и Action Panel во всех поверхностях                                       |         3 | Общий command UI kit                                                  |

Текущая реализация поиска подтверждается в
`platform/desktop/src/views/LauncherView.vue::scoreCommand` и
`platform/runtime/src/app_index/ranking.rs`. Clipboard выключен флагом
`CLIPBOARD_HISTORY_ENABLED = false`; причины заморозки описаны в
`docs-site/concepts/clipboard-history.md`.

## P1 — power-user parity

| Функция                                 | Что есть в Kosmos                                                           | Что требуется до Raycast-level                                                                                                                 | Сложность | Основная зависимость                  |
| --------------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------: | ------------------------------------- |
| System Commands                         | Настройки, update и несколько shell-команд                                  | Lock, sleep, restart, shutdown, empty trash, display/audio controls, confirmations, настройка видимости                                        |         3 | Безопасный OS bridge                  |
| Window Management                       | Общего window manager нет                                                   | Halves, thirds, quarters, maximize/restore, displays, next/previous display, custom layouts и gaps                                             |         4 | Win32/macOS window APIs               |
| Emoji & Symbols                         | Нет                                                                         | Unicode search, aliases, skin tones, pinned/recent, copy/paste, configurable output                                                            |         2 | Готовый Unicode dataset               |
| Script Commands                         | Есть command host и несколько command modes                                 | Watch folders, metadata format, args/env, output modes, templates, PowerShell/Node/Python, logs, permissions                                   |         4 | Изолированный process runtime         |
| Extension SDK                           | Есть `.kext`, manifests, permissions, marketplace и List/Detail/Actions     | Стабильный публичный SDK: List/Grid/Detail/Form, navigation, preferences, storage, deeplinks, menu bar, compatibility guarantees               |        5+ | Версионируемый SDK и UI runtime       |
| Безопасный runtime сторонних extensions | Установленные пользователем команды сознательно не исполняются без изоляции | Sandbox/process isolation, capability tokens, resource limits, signing, review/update policy, crash containment                                |         5 | Security boundary до расширения Store |
| Focus parity                            | Есть Pomodoro, Delphi task, time entries, widget и hosts domain block       | OS-level app blocking, block/allow mode, reusable categories, sleep-aware sessions, unlimited mode, snooze, восстановление закрытых приложений |         4 | Kosmos System Service                 |
| Dictation polish                        | Есть local/cloud STT, PTT, VAD, auto-paste, pending/retry                   | Vocabulary, app context, per-app styles, history, copy last, stats, Dictate to Eden Note                                                       |         4 | Context capture и безопасная история  |
| Eden Quick Capture                      | Есть `/new`, `/today`, глубокий редактор и дневник                          | Глобальные hotkeys, мгновенная floating note, pinned/recent stack, capture выделенного текста, прямая диктовка в запись                        |         2 | Общие aliases/hotkeys и Action Panel  |
| Auto Quit                               | Usage tracker уже видит процессы и foreground usage                         | Таймаут на приложение, safe quit, исключение foreground/media/recording apps, временная пауза                                                  |         3 | Текущий usage tracker                 |

Главный блокер extension ecosystem уже виден в
`platform/desktop/electron/command-host/command-runner.ts`: сторонняя команда
отклоняется без isolated runtime. Существующий command UI переписывать не
нужно — базовые List/Detail/Search/Actions уже есть в
`platform/desktop/src/command-host/`.

## P2 — полнота продукта

| Функция                    | Что есть в Kosmos                                                   | Что требуется до Raycast-level                                                                                              | Сложность | Основная зависимость                  |
| -------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | --------: | ------------------------------------- |
| Screenshot Search + OCR    | Общего screenshot index/OCR нет                                     | Scopes, date/name/content filters, on-device OCR, preview, paste latest, retention                                          |         4 | Windows OCR/WinRT или локальный OCR   |
| Поиск внутри файлов        | Есть filename index                                                 | Content index для поддерживаемых txt/md/pdf/docx, parsers, size/type limits, ignores, background reindex, privacy settings  |         5 | Отдельная indexing pipeline           |
| Theme Studio               | Есть дизайн-токены `@kosmos/visuals`                                | Редактор темы, независимые light/dark варианты, import/share, проверка extensions                                           |         3 | Соблюдение tokens всеми поверхностями |
| Полный import/export       | Есть отдельные экспорты данных ARK/Eden                             | Версионированный bundle: settings, aliases, hotkeys, snippets, quicklinks, extensions, themes; secrets отдельно; encryption |         4 | Общая settings schema и migrations    |
| Sync UX                    | ARK P2P/LAN/relay технически силён по ownership                     | Понятное pairing/recovery, device list, status, conflict UI, sync launcher settings, encrypted remote fallback              |         4 | Объединение ARK и shell settings      |
| Translate                  | Системной команды нет                                               | Определение языка, перевод выделенного/введённого текста, custom language-pair commands, copy/paste                         |         3 | Отдельный provider или AI layer       |
| General AI Chat / Quick AI | Daedalus управляет Codex, но не является универсальным AI assistant | Chat history, selected text/files, multiple models, local/BYOK, attachments, always-on-top, command generation              |         5 | Provider layer, secrets, chat storage |
| Browser context            | Полноценного browser companion нет                                  | Tabs/page/selection context, действия над страницей, отправка в AI/Eden, browser extension                                  |         4 | Browser extension и permission model  |

## P3–P4 — стратегическая ширина

| Приоритет | Функция                            | Что требуется до Raycast-level                                                                                          | Сложность | Решение сейчас                                       |
| --------- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | --------: | ---------------------------------------------------- |
| P3        | AI Extensions, MCP, Agents, Skills | Tool schemas, permissions, confirmations, MCP auth/lifecycle, agent runs, skills discovery, extension-provided AI tools |        5+ | Только после AI Chat и безопасного extension runtime |
| P3        | Calendar, meetings, contacts       | Google/Microsoft OAuth, agenda, join/create meeting, availability, contacts search                                      |         5 | Не строить до появления реального спроса             |
| P3        | Hyper Key                          | Переназначение Caps Lock/F-keys в modifier, combinations, conflict handling, secure-input degradation                   |         4 | Низкий ROI относительно обычных hotkeys              |
| P4        | Полная macOS parity                | Native window, clipboard, shortcuts, dictation permissions, focus blocking, packaging и QA второй платформы             |        5+ | Отдельная platform program                           |
| P4        | iOS/mobile companion               | AI, notes, snippets, quicklinks, dictation keyboard, widgets, Shortcuts, share extension                                |        5+ | Не часть launcher parity на Windows                  |
| P4        | Teams/enterprise/private Store     | Accounts, billing, orgs, roles, shared commands/snippets/quicklinks, private extensions, admin controls                 |        5+ | Не делать без смены продуктовой модели               |

## Рекомендуемая последовательность

| Шаг | Содержание                                                           | Почему в таком порядке                                                                   |
| --: | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
|   1 | Единый catalog/ranking, aliases, hotkeys, Action Panel               | Это общий фундамент, без которого остальные функции расползутся по отдельным реализациям |
|   2 | Quicklinks, Calculator, Emoji, File Search actions                   | Высокая ежедневная ценность при умеренной сложности                                      |
|   3 | Event-driven Clipboard, Snippets, System Commands, Window Management | Native-функции с более высоким риском, но уже на общей command/action модели             |
|   4 | Script Commands, isolated runtime, стабильный Extension SDK          | Сначала безопасный runtime, потом расширение ecosystem                                   |
|   5 | Focus, Dictation и Eden Quick Capture                                | Усиление того, где у Kosmos уже есть собственная сильная база                            |
|   6 | Content/OCR, AI Chat, MCP/Agents                                     | Дорогие подсистемы после стабилизации launcher core                                      |
|   7 | Calendar, macOS/iOS, Teams                                           | Только при подтверждённой продуктовой необходимости                                      |

## Что не входит в parity backlog

В список не включены области, где Kosmos уже концептуально сильнее Raycast:

| Область                       | Преимущество Kosmos                                               |
| ----------------------------- | ----------------------------------------------------------------- |
| Local-first данные            | ARK, P2P/LAN/relay и владение пользовательскими данными           |
| Заметки                       | Eden: typed notes, дневник, связанные объекты и глубокий редактор |
| Задачи и фокус                | Связка Focus → Delphi task → time entry                           |
| Диктовка                      | Выбор локальных моделей и детальный контроль delivery pipeline    |
| Специализированные приложения | Arrancador, Akasha и Daedalus                                     |

---

# P0 повторно: нужен ли он вообще?

Эта секция предназначена для принятия решения. «Минимальный вариант» — первая
версия, которую стоит проверить на себе до разработки полного Raycast parity.

## P0.1 — единый fuzzy-поиск

| Вопрос                | Ответ                                                                                                                                                         |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Kosmos должен находить нужное, даже если пользователь написал неточно, не с начала названия или с опечаткой                                                   |
| Юзкейс                | Пользователь пишет `vsco`, получает Visual Studio Code; пишет `fierf`, получает Firefox; часто запускаемый Delphi поднимается выше редко используемой команды |
| Что раздражает сейчас | Приложения и команды ранжируются разными алгоритмами; команда с опечаткой не находится; порядок результатов ощущается непредсказуемым                         |
| Минимальный вариант   | Общий scorer поверх существующего каталога: exact → alias → prefix → word prefix → fuzzy → subtitle, затем небольшой frecency boost                           |
| Не делать, если       | Launcher почти всегда используется через избранное или точные запросы и реальные промахи поиска не наблюдаются                                                |
| Сигнал «делать»       | За неделю регулярно приходится стирать запрос, вводить полное имя или пролистывать неправильный top result                                                    |
| Вердикт               | **Оставить P0.** Это базовое качество launcher, а app frecency уже существует и может быть переиспользовано                                                   |

## P0.2 — aliases команд

| Вопрос                | Ответ                                                                                                                            |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Пользователь сам задаёт короткое имя длинной или плохо названной команде                                                         |
| Юзкейс                | `t` → создать задачу; `n` → новая заметка; `upd` → проверить обновления; русское `фокус` находит англоязычную внутреннюю команду |
| Что раздражает сейчас | Нужно помнить официальное название и язык каждой команды                                                                         |
| Минимальный вариант   | Одно необязательное alias-поле на команду, редактирование только в Settings, участие в общем ranking                             |
| Не делать, если       | Названий команд мало, они короткие и пользователь никогда не ищет их другими словами                                             |
| Сигнал «делать»       | Появляются команды, для которых пользователь помнит действие, но не системное название                                           |
| Вердикт               | **Оставить P0**, но не строить теги, группы и синхронизацию до появления общего settings export                                  |

## P0.3 — hotkey для любой команды

| Вопрос                | Ответ                                                                                                                      |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Частое действие запускается сразу, без открытия Kosmos и поиска                                                            |
| Юзкейс                | `Ctrl+Alt+N` создаёт заметку, `Ctrl+Alt+T` — задачу, отдельная комбинация запускает Focus Session или конкретный Quicklink |
| Что раздражает сейчас | Для каждого нового глобального действия разработчик должен вручную добавлять отдельный hotkey path                         |
| Минимальный вариант   | Один shortcut на команду, Windows-only, конфликт с уже занятым shortcut показывает ошибку; без chord sequences и Hyper Key |
| Не делать, если       | Пользователь предпочитает один launcher hotkey и поиск занимает меньше секунды                                             |
| Сигнал «делать»       | Есть 3–5 действий, которые запускаются много раз в день и каждый раз требуют открыть launcher                              |
| Вердикт               | **Условный P0.** Делать после aliases и command identity; если одного launcher hotkey достаточно, понизить до P1           |

## P0.4 — универсальный Action Panel

| Вопрос                | Ответ                                                                                                                                                                   |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | У любого результата есть предсказуемое меню возможных действий, открываемое одной клавишей                                                                              |
| Юзкейс                | На файле: открыть, скопировать путь, показать в Explorer, удалить; на приложении: запустить, открыть папку, закрепить; на заметке: открыть, скопировать ссылку, удалить |
| Что раздражает сейчас | Root launcher и command host предлагают разный набор и разное поведение действий; новые команды реализуют меню заново                                                   |
| Минимальный вариант   | Переиспользовать богатую модель command host в launcher; одна плоская секция actions и shortcut hints; вложенность добавить только при переполнении                     |
| Не делать, если       | У каждого результата действительно существует только одно действие — открыть                                                                                            |
| Сигнал «делать»       | В UI появляются отдельные кнопки или контекстные меню для copy path, delete, favorite и других вторичных действий                                                       |
| Вердикт               | **Оставить P0.** Это уменьшает будущий код: один action path дешевле отдельных UI для каждой фичи                                                                       |

## P0.5 — File Search actions и preview

| Вопрос                | Ответ                                                                                                                            |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | File Search должен не просто находить путь, а позволять закончить действие, не открывая Explorer                                 |
| Юзкейс                | Найти PDF, посмотреть дату и размер, открыть папку; найти скриншот и сразу вставить; скопировать абсолютный путь в терминал      |
| Что раздражает сейчас | Индекс уже сильный, но результат почти всегда только открывается; для остальных действий приходится снова искать файл в Explorer |
| Минимальный вариант   | Detail pane с path/size/date и actions: Open, Show in Explorer, Copy Path; Quick Look, trash и Open With добавить позже          |
| Не делать, если       | File Search используется только как быстрый способ открыть документ                                                              |
| Сигнал «делать»       | После нахождения файла пользователь регулярно открывает Explorer ради пути, свойств или выбора приложения                        |
| Вердикт               | **Оставить P0 в минимальном виде.** Полный файловый менеджер не нужен                                                            |

## P0.6 — Clipboard History

| Вопрос                | Ответ                                                                                                                                                                  |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Можно вернуть то, что копировалось несколько операций назад                                                                                                            |
| Юзкейс                | Скопировать логин, код и ссылку по очереди, затем вставить каждый элемент; найти команду из терминала, скопированную десять минут назад; повторно вставить изображение |
| Что раздражает сейчас | Буфер Windows хранит ограниченную историю и не интегрирован с командами Kosmos; существующая Kosmos-реализация выключена из-за performance риска                       |
| Минимальный вариант   | Сначала проверить, достаточно ли системного `Win+V`. Если нет — event-driven text-only history, поиск, paste и исключение password managers; изображения позднее       |
| Не делать, если       | `Win+V` полностью покрывает сценарии и интеграция с Kosmos не даёт измеримой пользы                                                                                    |
| Сигнал «делать»       | Нужны keyboard-first search, приложение-исключения, pin/edit или общий sync с Kosmos                                                                                   |
| Вердикт               | **Понизить до условного P0.** Сначала использовать системный `Win+V`; собственный Clipboard оправдан только подтверждёнными ограничениями                              |

## P0.7 — Quicklinks

| Вопрос                | Ответ                                                                                                                                                                               |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Любой URL, файл или deeplink становится собственной командой с параметром                                                                                                           |
| Юзкейс                | `gh kosmos` открывает GitHub search по репозиторию; `yt synthwave` открывает поиск YouTube; `issue 123` открывает конкретный issue; `docs ark` открывает нужный раздел документации |
| Что раздражает сейчас | Повторяющиеся URL приходится хранить в bookmarks или вручную собирать каждый раз                                                                                                    |
| Минимальный вариант   | Название + URL template с одним `{query}` + необязательный alias; всё остальное отложить                                                                                            |
| Не делать, если       | Есть только несколько статичных ссылок, которые уже удобно лежат в браузерных bookmarks                                                                                             |
| Сигнал «делать»       | Один и тот же сайт регулярно открывается с разными поисковыми параметрами                                                                                                           |
| Вердикт               | **Оставить P0.** Это дешёвая функция с большим leverage и почти без native риска                                                                                                    |

## P0.8 — Calculator и conversions

| Вопрос                | Ответ                                                                                                                                                                                                 |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Статус                | **Минимальный вариант реализован:** `fend-core` в Rust runtime, live-preview в общей выдаче, copy result и валюты с ежедневным кешем курсов ЦБ РФ                                                     |
| Простыми словами      | Launcher отвечает на короткий расчёт прямо в строке поиска                                                                                                                                            |
| Юзкейс                | `1200*1.2`, `15% of 4500`, `10 km in miles`, `18:00 Tokyo in Moscow`, `100 usd in rub`                                                                                                                |
| Что раздражает сейчас | Для мелкого расчёта нужно открывать Calculator или браузер, теряя текущий контекст                                                                                                                    |
| Минимальный вариант   | Реализован: арифметика, проценты, unit conversions, системы счисления, даты, currency и базовая русская валютная форма через `fend-core`; timezone и широкий natural language — отдельно после спроса |
| Не делать, если       | Windows Calculator вызывается hotkey и переключение контекста не раздражает                                                                                                                           |
| Сигнал «делать»       | Мелкие вычисления выполняются несколько раз в день во время работы с launcher                                                                                                                         |
| Вердикт               | **P0 закрыт маленьким calculator.** Colors, timezone и сложный natural language — P2/YAGNI                                                                                                            |

## P0.9 — Snippets и text expansion

| Вопрос                | Ответ                                                                                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Часто повторяемый текст вставляется по короткой команде или ключевому слову                                                                       |
| Юзкейс                | Вставить адрес, шаблон ответа, Markdown-блок, команду запуска, подпись или prompt с текущей датой                                                 |
| Что раздражает сейчас | Повторяемый текст хранится в случайных заметках и каждый раз копируется вручную                                                                   |
| Минимальный вариант   | Сначала searchable snippets как обычные Kosmos commands: выбрать → вставить. Автоматическое expansion по набранному keyword добавить только потом |
| Не делать, если       | Повторяемых текстов мало или они уже удобно хранятся в Eden и вставляются редко                                                                   |
| Сигнал «делать»       | Один и тот же фрагмент копируется несколько раз в неделю либо нужны placeholders                                                                  |
| Вердикт               | **Searchable snippets — P0/P1 и дёшево. Auto-expansion — P1**, потому что native hook резко повышает риск                                         |

## P0.10 — единая keyboard-first навигация

| Вопрос                | Ответ                                                                                                                                                                        |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Простыми словами      | Enter, Escape, стрелки и Action Panel работают одинаково во всех командах                                                                                                    |
| Юзкейс                | Пользователь открывает File Search, Focus или extension command и не задумывается, какой клавишей вернуться, выполнить основное действие или открыть дополнительные действия |
| Что раздражает сейчас | Каждая новая поверхность может получить собственные shortcuts, loading и empty states                                                                                        |
| Минимальный вариант   | Зафиксировать общий keyboard contract и переиспользовать существующие command-host компоненты; не создавать новый UI framework                                               |
| Не делать, если       | Все текущие поверхности уже ведут себя одинаково по ручной проверке                                                                                                          |
| Сигнал «делать»       | Пользователь нажимает ожидаемую клавишу и ничего не происходит либо поведение отличается между командами                                                                     |
| Вердикт               | **Оставить P0 как правило качества, не как отдельный большой проект.** Исправлять по мере переноса функций на общий command UI                                               |

## Решение по P0 после проверки необходимости

| Решение                                              | Функции                                                                                                           |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Делать первыми                                       | Единый поиск, aliases, Action Panel, Quicklinks, минимальный Calculator                                           |
| Делать минимально на существующей базе               | File Search metadata/actions, searchable Snippets, единый keyboard contract                                       |
| Делать только при подтверждённом ежедневном сценарии | Глобальные per-command hotkeys                                                                                    |
| Сначала проверить системную альтернативу             | Clipboard History против `Win+V`                                                                                  |
| Пока не делать                                       | Clipboard OCR/QR/sequential paste, snippet auto-expansion, currency/colors/natural-language calculator, Hyper Key |

Минимальный рациональный scope — не десять независимых подсистем, а пять
изменений: общий ranking, aliases, общий Action Panel, Quicklinks и простой
Calculator. Остальные P0 либо расширяют уже существующий File Search/command
host, либо должны пройти проверку реальным использованием.
