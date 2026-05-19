# Changelog

Все заметные изменения Kepler launcher. Формат — [Keep a Changelog](https://keepachangelog.com/), версии — semver. Источник правды для post-update модалки и release notes на GitHub.

Поддерживается вручную: при каждом version bump'е в `shell/package.json` добавляется новая секция с датой и списком изменений.

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
