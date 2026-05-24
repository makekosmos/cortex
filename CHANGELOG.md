# Changelog

Все заметные изменения Kepler launcher. Формат — [Keep a Changelog](https://keepachangelog.com/), версии — semver. Источник правды для post-update модалки и release notes на GitHub.

Поддерживается вручную: при каждом version bump'е в `shell/package.json` добавляется новая секция с датой и списком изменений.

## [0.3.1] — 2026-05-25

Полировка диктации поверх 0.3.0 — фиксы латентности, системные shortcut'ы, визуал pill.

### Добавлено

- **Захват системных shortcut'ов как hotkey** (Win+H, Win+Space и т.п.). Settings → Диктация → Горячая клавиша теперь использует **system-level capture mode** в нашем WH_KEYBOARD_LL hook'е (ARK op `dictation.begin_hotkey_capture` / `end_hotkey_capture`). Backend ловит nажатие ДО того как Voice Typing / layout switcher успеют среагировать.
- **`HookMode::Toggle`** для hotkey hook'а: hook теперь используется и в Toggle mode (раньше только PTT). Все accelerator'ы идут через intercept-вариант → Electron `globalShortcut` больше не нужен. Системные Win+X shortcut'ы заменяются нашим действием на всё время работы Kepler'а.
- **Persistent audio stream с 30s idle keep-alive** — `MediaStream` + `AudioContext` остаются открытыми между сессиями. Серия диктовок подряд → ~0ms latency (нет повторного `getUserMedia`). Через 30s тишины track останавливается, Windows mic indicator гаснет.
- **Idle warmup pill window** — `BrowserWindow` создаётся скрытым через 3s после старта Kepler через `setImmediate`. Первое нажатие hotkey — мгновенно.
- **Запрет single-key hotkey'ев** в capture mode — Windows-конвенция «hotkey = modifier + key». Одиночная буква игнорируется, ждёт следующее нажатие.

### Изменено

- **Pill визуал** (`shell/src/views/DictationPillView.vue`): 240×72 окно, 200×56 pill, снизу экрана 100px от низа. Чёрный glossy с тонким inset bevel, без drop-shadow. Внутри — только waveform / dots / `!`, без таймера и подписей.
- **Default модель** — `whisper-large-v3` (не turbo). Turbo даёт пропуски слов и галлюцинации на русском.
- **Settings → Диктация** — UI упрощён: убраны row'ы «Тест записи», «Контекстный prompt» (теперь захардкожен), выбор модели. Микрофон вынесен в отдельную секцию с заголовком (как Статистика).

### Исправлено

- **Inject `TryFromIntError(())`** — `enigo::Key::Unicode('v')` шёл через VK_PACKET (Unicode channel), модификаторы там не работают как shortcut, enigo падал при упаковке keystate в u32. Заменено на нативный `windows::Win32::SendInput` с `VK_CONTROL` + `VK_V` (0x56). Зависимость `enigo` удалена.
- **Pill race condition** — `start` команда улетала в renderer до `onMounted` Vue компонента. Добавлен `pillReady: Promise<void>` (резолвится на `did-finish-load` + 50ms tick).
- **Capture модификаторов не работал в `HotkeyCapture`** — hook intercept'ил modifier-нажатия, Windows не успевал обновить keyboard state, `GetAsyncKeyState(VK_LWIN/...)` возвращал false. Modifier-нажатия теперь пропускаются через `CallNextHookEx`; intercept только non-modifier finals.
- **Start menu открывался после Win+H intercept'а** — Win down→up без других клавиш между ними триггерит Start. Стандартный AHK/PowerToys приём: после intercept'а посылаем dummy `SendInput` с VK_RESERVED (0xFF) → Windows считает Win использованной как modifier, Start не открывается.
- **`update_config` игнорировал `provider`/`model`** — UI слал patch, backend терял поля.
- **Очищены 4 dead-code warning'а** в `file_index/store.rs` (поле `StatsSnapshot.ignore_patterns`, метод `count_files`) и `file_index/watcher.rs` (regression-функции получили `#[allow(dead_code)]` с обоснованием).

### Заметки

- Hook intercept глобален: пока Kepler запущен, выбранный hotkey **не работает ни в одном другом приложении**. На shutdown ОС автоматически снимает hook. Это by design.
- Системный Voice Typing на Win+H остаётся **установленным**, мы лишь перехватываем событие до него. Никаких registry-изменений / отключений системного у пользователя не происходит.

## [0.3.0] — 2026-05-25

Голосовой ввод (диктация) по образцу Raycast — global hotkey → плавающая pill → транскрипция в Groq Cloud → авто-вставка в активное окно.

### Добавлено

- **Dictation модуль** (`services/kepler-backend/src/dictation/`): host (state machine), Groq client, inject через нативный Win32 `SendInput` (Ctrl+V), keyring для API-ключа, DoH/proxy для AI HTTP, low-level keyboard hook для push-to-talk, persistent stats (WPM / Total Words / Time Saved).
- **Pill window** — frameless, alwaysOnTop, `focusable: false`, снизу экрана 100px от низа, glossy чёрный визуал с waveform. Audio capture в renderer через Web Audio API (16kHz mono PCM → WAV → base64).
- **Settings → Безопасность** — DNS-резолвер для AI-провайдеров (System / Cloudflare DoH / Google DoH / Custom DoH) + HTTP/SOCKS proxy + кнопка проверки соединения.
- **Settings → Секреты** — управление Groq API key (Windows Credential Manager через `keyring`).
- **Settings → Диктация** — статистика, выбор микрофона, язык (23 варианта), горячая клавиша, режим триггера (Toggle / PTT), способ инжекта (Auto-paste / Clipboard only), выбор поставщика.
- **Anti-hallucination**: `language` hint + `temperature=0` + `verbose_json` segment filtering (`no_speech_prob > 0.6`, `avg_logprob < -1.0`) + hardcoded короткий prompt с domain-терминами.
- **`@kosmos/visuals`** — 6 новых компонентов: `Button`, `TextInput`, `Textarea`, `RadioGroup`, `HotkeyCapture`, общий primitive-набор для settings.

### Изменено

- **Settings window** теперь имеет `minWidth: 800, minHeight: 560`.
- **Sidebar порядок:** Main = Общие → Безопасность → Секреты → Дебаг → О приложении. Advanced = Поиск файлов → Расширения → Диктация → Фокус → Задачи → Заметки → Времяметр → Игры.
- **Settings → Общие** — кнопка hotkey'я launcher'а перевязана на общий `HotkeyCapture`.

### Исправлено

- **Pill race condition** — при первом запуске `start` команда уходила в renderer до `onMounted` Vue компонента. Добавлен `pillReady: Promise<void>` (резолвится на `did-finish-load` + 50ms tick).
- **Inject `TryFromIntError`** — `enigo::Key::Unicode('v')` отправлял `v` через VK_PACKET (Unicode-канал), на котором модификаторы не работают как shortcut, и сам enigo падал. Заменено на нативный `windows::Win32::SendInput` с `VK_CONTROL` + `VK_V` (0x56).
- **`update_config` игнорировал `provider`/`model`** — UI слал patch, backend терял поля.

### Заметки

- Default модель — `whisper-large-v3` (не turbo). См. `docs-site/concepts/dictation.md` § «Выбор модели» — turbo даёт пропуски слов и больше галлюцинаций на русском.
- Default hotkey — `Ctrl+Shift+;`. На русской раскладке `;` отсутствует (там `ж`) — Electron `globalShortcut` маппит по scan-code OEM_1, всё равно работает, но визуально юзер может видеть это как `Ctrl+Shift+~` (label ОС). Поменять в Settings → Диктация.
- Roadmap: локальные модели (whisper.cpp / Parakeet) — Phase 2. LLM post-processing — Phase 3. Silero-VAD предобработка — отдельно.

## [0.2.0] — 2026-05-19

Milestone-релиз, объединяет работу `0.1.20` и `0.1.21` под единым тегом и фиксирует расширение публичного API для extension-разработчиков.

### Изменено

- **Bump `KEPLER_API_VERSION` 1.0.0 → 1.1.0** (additive). Extensions могут просить `keplerApiVersion: "^1.1.0"` чтобы заявить «мне нужны `manifest.commands[]` + dynamic `commands.register`».

### Включено из 0.1.20

- **Three-layer command architecture.** Extension теперь объявляет launcher-команды в `manifest.json` (V1 manifest-declared) либо регистрирует их runtime через `commands.register` WS-операцию (V2 dynamic). Shell мержит три источника (internal > manifest > dynamic) с dedup по id и namespace-prefix `${manifest.id}:`.
- **Auto-launch action-команд.** Invoke action-команды у не запущенного extension'а поднимает его и dispatch'ит после mount (`awaitExtensionCommand` с 5s timeout).
- **`commands_changed` event на bus** — подписка из renderer'а через `window.kepler.commands.onCommandsChanged`.

### Включено из 0.1.21

- **Zero-UAC focus mode.** При первой активации блокировки Kepler автоматически устанавливает `KeplerFocusSvc` (Windows Service, AutoStart) — один UAC сейчас, ноль UAC потом. Все последующие активации идут через named pipe `\\.\pipe\kepler-focus-svc`. Карточка «Системный демон» в Settings → Фокус с install/переустановить/удалить.

## [0.1.21] — 2026-05-19

### Изменено

- **Focus mode без постоянного UAC.** При первой активации блокировки Kepler один раз спросит права администратора и установит системный демон `KeplerFocusSvc` (AutoStart). Все последующие активации pomodoro/focus идут через named pipe — без запроса прав. В Settings → Фокус добавлена карточка «Системный демон» с кнопками установить / переустановить / удалить и статусом.

## [0.1.8] — 2026-05-16

### Добавлено

- Большая Update-плашка в launcher с заголовком + описанием изменений. Появляется при наличии скачанного обновления.
- Дефолтный глобальный хоткей `Alt+Space` (раньше `Ctrl+Shift+K`). В Settings → Общие можно нажать на хоткей и записать своё сочетание; кнопка «Сброс» возвращает дефолт.
- Кнопка «Проверить обновления» переехала в нижний левый угол вкладки Расширения (footer под списком).
- Стрелки ↑/↓ в launcher теперь оставляют 8px зазор от верхнего/нижнего края списка перед скроллом.
- CHANGELOG.md в корне репо.

### Изменено

- Show/hide launcher без Windows DWM fade — окно держится живым и прячется off-screen с `opacity:0`. По хоткею — моментально появляется.
- Анимация выделения плашки убрана — выделение моментальное.
- Tray menu: убран пункт «Dashboard», остались «Открыть» / «Настройки» / «Выход».
- Команда `Проверить обновления` больше не открывает Settings — просто запускает `autoupdater.check()`. Результат показывается через update banner в launcher.

## [0.1.7] — 2026-05-16

### Добавлено

- Pinned «Обновление» tile в launcher с Lucide `ArrowUpCircle` / `Loader2`, сплошной синий фон. Enter / клик в `downloaded` состоянии — `quitAndInstall`.
- Section header reveal: при выборе первого item секции прокрутка целится в `.section-label` (`block: "start"`), header полностью виден с верхним отступом.

### Изменено

- Arrow navigation в launcher теперь **clamp**, без wrap — упор в начало/конец списка.
- «Все» секция в launcher содержит весь список включая команды из «Недавние» (дубли намеренные).

## [0.1.6] — 2026-05-16

### Добавлено

- Vue компонент `BuiltInIcon` с пропсами `icon`/`from`/`to`/`size`/`strokeWidth`. Дефолт — голубой → синий gradient + `HelpCircle`.
- Новые команды: `settings:open`, `kepler:check-updates`, `delphi:today`, `horologion:pomodoro`, `horologion:stopwatch`.
- `CommandRecord` расширен полями `kind` / `appName` — UI рендерит «Команда · App» справа от title.
- Секции «Недавние» (5 последних, localStorage) и «Все» в launcher.
- Hide-on-blur (кроме DevTools focus в dev mode).
- Scroll-driven fade scrollbar в `@kepler/visuals` через `@property --kosmos-scroll-alpha`.
- Глубокие ссылки в extension'ы через IPC `kepler:extension:navigation` — работает с любым `vue-router` history mode (memory / hash / web).
- Horologion `HomeView` читает `route.query.mode` → переключает `timerMode` (Pomodoro / Секундомер).

### Изменено

- `isDeveloperModeActive()` больше не читает `KEPLER_DEV` env — extension HMR требует только Settings toggle.
- Settings окно — 880×560 resizable.
- Settings → Расширения — плоский список установленных, без catalog scaffolding.

### Удалено

- Команда `eden:open`. Eden остаётся standalone, но в launcher'е больше не показывается.

## [0.1.5] — 2026-05-15

### Добавлено

- Lucide-иконки для built-in команд (Settings, Dashboard) с gradient squircle.
- Команда `settings:open`.
- Settings → Расширения: merged единый список (раньше было два tab'а — Установленные / Маркетплейс).

### Изменено

- «Открыть Dashboard» → «Открыть таблицу данных», subtitle «Просмотр объектов ARK».
- Settings window — увеличен до 880×560 и сделан resizable.

## [0.1.4] — 2026-05-15

### Добавлено

- Manual «Проверить обновления» в Settings → General (вызывает `autoupdater.check()`).

## [0.1.3] — 2026-05-15

### Исправлено

- Tray icon в production: `resolveTrayIconPath` теперь сначала проверяет `process.resourcesPath/icon.png` (загруженный через `extraResources` electron-builder).

## [0.1.2] — 2026-05-15

### Добавлено

- Raycast-style update banner в Settings (32px sticky bar, Lucide `ArrowUpCircle` / `Loader2`).
- IPC `kepler:settings:update:{check,install,state}` + state subscription.

## [0.1.1] — 2026-05-15

### Исправлено

- NSIS требует `.ico` (не `.png`) для installer/uninstaller icons.
- `win.icon` → PNG (electron-builder auto-генерит full-size `.ico` под Windows resource).

## [0.1.0] — 2026-05-15

### Первый публичный релиз

- Electron launcher с глобальным хоткеем.
- Command bus (in-memory registry + WS-операции `commands.{register,unregister,list,invoke}` через kepler-backend).
- Extension host (Vue extensions с manifest + dev mode + .kext установка).
- Dashboard view (встроенный ARK browser).
- Settings окно с autostart toggle + версия + backend-статус.
- `electron-updater` интеграция с GitHub Releases (`yoso-industries/kepler-releases`).
- Marketplace (catalog.json из `yoso-industries/kosmos-extensions`).

[0.1.8]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.8
[0.1.7]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.7
[0.1.6]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.6
[0.1.5]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.5
[0.1.4]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.4
[0.1.3]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.3
[0.1.2]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.2
[0.1.1]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.1
[0.1.0]: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.1.0
