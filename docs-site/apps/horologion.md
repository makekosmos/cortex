# Horologion — трекер времени

::: tip Статус
**MVP работает.** Единая Home-страница: pomodoro/секундомер с переключателем в шайбе (sliding pill), плавная анимация смены режимов (swipe + height transition card'а), @-mention задач Delphi (в т.ч. мульти-задачи в помодоро с равномерным дроблением сегмента), live duration в списке, edit-modal, ПКМ-удаление, группировка одинаковых, collapse/expand дней, помодоро со звуками и системными уведомлениями, tray + close-to-tray, NSIS one-click установщик, embed'нутая иконка в `.exe`.
:::

::: info Имя
Имя приложения — **Horologion** (греч. ὡρολόγιον — «часослов»). Workspace-директория исторически осталась `apps/horologion`; внутренние идентификаторы (`HorologionApi`, `window.horologion`, IPC `horologion:*`) тоже сохранены, чтобы не ломать git-историю и type-graph. Меняется только всё user-visible: `productName`, `appId` (`com.kazui.horologion`), AppUserModelID, NSIS shortcut, title окна, текст в trail/tray.
:::

- **Path**: `apps/horologion`
- **Стек**: Electron 41 + Vite 8 + **Vue 3.6 Vapor** + `@kosmos/ark` + `@kosmos/visuals`. Жёсткое окно 600×800px.
- **Аналог**: Toggl Track — без социалки, без web-app, локально, с интеграцией Delphi-задач.

## Список фич, которые планируется/нужно сделать

См. [Roadmap](/apps/horologion-roadmap).

## Структура `src/`

```
apps/horologion/
├─ electron/
│  ├─ main.ts              # ArkClient sidecar, IPC, BrowserWindow, tray, settings window
│  └─ preload.ts           # contextBridge → window.horologion
├─ shared/
│  └─ ipc-types.ts         # HorologionApi + TimeEntry/Tag/DelphiTask types
├─ src/
│  ├─ App.vue              # DesktopChrome + RouterView + scroll fade. На /settings рендерит только SettingsView
│  ├─ main.ts              # Vue createApp + Inter Variable import + router mount
│  ├─ router.ts            # `/` → HomeView, `/settings` → SettingsView (для отдельного окна)
│  ├─ styles.css           # --horologion-accent + локальные токены
│  ├─ views/
│  │  ├─ HomeView.vue      # Draft input + Pomodoro/Stopwatch toggle + список
│  │  ├─ PomodoroView.vue  # Ring + ticks + dots + actions; читает usePomodoro
│  │  ├─ StopwatchView.vue # Большое HH:MM:SS + primary-кнопка
│  │  ├─ ListView.vue      # Группы по дням, collapse/expand, live duration tick
│  │  └─ SettingsView.vue  # Группы настроек, открывается в отдельном окне
│  ├─ components/
│  │  ├─ PomodoroDraftInput.vue  # Поле «Над чем работаем?» с chip'ами задач
│  │  ├─ MentionInput.vue        # Generic @-mention обёртка
│  │  ├─ MentionMenu.vue         # Popover автокомплита задач
│  │  └─ EditEntryModal.vue      # Modal редактирования time_entry
│  └─ lib/
│     ├─ store.ts          # entriesChangedAt signal, pomodoroDraft, tasks cache, timerMode
│     ├─ usePomodoro.ts    # state machine pomodoro (singleton)
│     ├─ pomodoroSettings.ts  # настройки помодоро (localStorage)
│     ├─ sounds.ts         # звуки конца work/break
│     └─ format.ts         # formatDuration / dayKey / formatDayHeader
└─ build/
   ├─ icon.png             # 1024×1024 PNG
   ├─ icon.ico             # cache, генерируется afterPack'ом
   └─ afterPack.cjs        # embed icon в Horologion.exe через rcedit
```

## UI и дизайн

Horologion полностью использует [`@kosmos/visuals`](/packages/kosmos-visuals): `<DesktopChrome>` + `<DesktopContentSurface>` обёртка, все цвета / шрифты / радиусы — только через CSS-переменные kosmos-visuals. **Никакого hardcoded `#hex` или собственного titlebar-кода.**

### Структура окна

```
┌────────────────────────────────────────────────┐
│ Horologion          ● ARK status  ⚙  [─][□][✕]  │  titlebar
├────────────────────────────────────────────────┤
│  ╭──────────────────────────────────────────╮  │
│  │  Над чем работаем?  @ для задачи         │  │  draft input card
│  ╰──────────────────────────────────────────╯  │
│  ╭──────────────────────────────────────────╮  │
│  │      ⟨ Помодоро │ Секундомер ⟩  ← pill   │  │  timer card
│  │              ◯  25:00                     │  │
│  │       [▶ Начать сессию]                   │  │
│  │       [Пропустить][Стоп]                  │  │
│  ╰──────────────────────────────────────────╯  │
├────────────────────────────────────────────────┤
│  ╭─ Ср, 13 мая ◀                    2:15:42 ╮  │
│  │ [3] Учёба @Vapor                 1:30:00 │  │
│  │     E2E тесты                    0:25:00 │  │
│  ╰──────────────────────────────────────────╯  │
└────────────────────────────────────────────────┘
```

### Titlebar
- **Слева**: «Horologion» (muted color, secondary).
- **Справа**: круглый dot подключения к ARK (`var(--status-success)` / warning / `var(--destructive)`) + ⚙ Настройки (открывает [отдельное окно настроек](#окно-настроек)).
- Windows-controls справа от наших иконок (через `titleBarOverlay`).

### Draft input card
- Скруглённый прямоугольник с inline-chip'ами выбранных задач + текстовый ввод.
- `@` запускает MentionMenu, выбор задачи добавляет chip; Backspace на пустом — убирает последний chip.
- Submit (Enter) на этом поле → запускает текущий выбранный режим таймера (помодоро или секундомер).

### Timer card (pomodoro / stopwatch)
- Сверху — segmented control (шайба) с двумя кнопками **Помодоро / Секундомер** + sliding accent pill, перекатывается transform-анимацией.
- Ниже — выбранный таймер: pomodoro (с ring + ticks 60×1 минута) или stopwatch (большое `HH:MM:SS`).
- Переключение режимов — swipe-анимация (translateX + opacity) с одновременной плавной анимацией высоты card'а (JS-driven, FLIP через inline `height`).
- При активной сессии (`isSessionActive`):
  - Неактивная кнопка в шайбе сжимается до 0 (max-width + padding + opacity), pill растягивается на полную ширину toggle'а (toggle стабилен за счёт `min-width: 224px` → нет snap'а в конце).
  - Draft input card и timer card визуально объединяются: gap → 0, соседние углы выпрямляются (`border-bottom-radius` у draft и `border-top-radius` у pomo транзишнятся в 0 + прилегающие border-color → transparent).

### Список записей
- Группировка по дням (clickable header `▼ Ср, 13 мая` + сумма). Клик по header'у — collapse/expand дня с CSS-grid анимацией `grid-template-rows: 1fr ↔ 0fr`.

### Список записей
- Группировка по дням (хедер с суммой).
- Группировка одинаковых entries (одинаковый title + taskId + billable) внутри дня → одна строка с `[N]` badge'ем.
- **Клик на `[N]`** → раскрывает группу (CSS Grid 0fr→1fr транзишн, 280ms) — видны индивидуальные подстроки `HH:MM — HH:MM`.
- **Клик на строку** → открывает Edit-modal.
- **ПКМ** на строке → context menu «Удалить» (групповое удаление всех entries в группе).
- `@TaskName` в title рендерится в accent-цвете (без `@`).
- $-badge для billable.

### Edit modal
- `<Modal>` из kosmos-visuals.
- Поле «Описание» — `<MentionInput>` (можно поменять / добавить задачу через `@`).
- Preview под input'ом показывает task-pill.
- Два `<DateTimePicker>` (С / По) — кастомный недельный календарь + текстовый ввод HH:MM.
- Кнопки: «Удалить» (слева, danger) / «Отмена» / «Сохранить».

### Помодоро
- Круговой SVG-таймер с tick-метками минут + крупный mono-счётчик `MM:SS`.
- Cвой `<MentionInput>` сверху — выбираешь «над чем работаешь» (можно поменять в любой момент, в т.ч. во время break'а — следующий work возьмёт новое значение).
- Точки `[● ● ○ ○]` показывают сколько помидорок до длинного перерыва.
- Кнопки: primary «Начать сессию» (по статусу: Пауза / Продолжить / Старт фокуса / Старт перерыва), Skip, Stop.
- **Состояние сохраняется при сворачивании в трей** — таймер тикает в фоне.

### Settings
- Длительности (work / short / long), сколько помидорок до длинного.
- 5 toggle'ей: трекать брейки как «Отдых», автостарт work, автостарт break, системные уведомления, режим стримера.
- Звук конца work и конца break — 4 опции через Web Audio (без файлов): «Колокольчик» / «Перелив» / «Стук» / «Сигнал» + ▶ для прослушивания.
- Громкость рингтона — slider 0–100%, общий множитель ко всем тонам.

## Объектная модель в ARK

| Тип | Где владеется | Роль |
|---|---|---|
| `time_entry_obj` | Horologion | Запись отрезка времени. propsJson: `startedAt`, `endedAt`, `source`, `billable`, `taskId`, `taskTitle`. |
| `tag_obj` | shared (Horologion / Delphi) | Общий тег. **Пока не используется в UI** (TODO). |
| `task_obj` | Delphi | Существующий тип, Horologion ссылается через `propsJson.taskId` (object_link — TODO). |

**Pomodoro не маркирует записи** — поле `kind` снято. Pomodoro чисто UI-фича, создаёт обычные `time_entry_obj` (опционально break-entries с title «Отдых», если `trackBreaksAsRest` включён в Settings).

## Topbar (extension)

В Horologion-extension'е (`apps/kepler-shell/extensions/horologion/`) topbar содержит два UI-элемента справа от заголовка:

- **Status dot** — круглая точка (8px), цвет показывает состояние подключения к `kepler-backend`. Логика в `extensions/horologion/src/App.vue`: при mount и каждые 10 секунд дёргает дешёвую операцию `kepler.ark.request("list_object_types")` — успех → `connected` (зелёный), ошибка → `error` (красный), стартовое состояние → `connecting`. Tooltip переключается между «ARK подключен» / «Подключение к ARK…» / «ARK недоступен». Визуально совпадает с Delphi extension status dot (одни и те же oklch-токены из `@kosmos/visuals`).
- **Кнопка ⚙ Настройки** — `router.push("/settings")` в memory-router'е extension'а. На route `/settings` App.vue прячет dot и кнопку, показывает «Назад» (`router.push("/")`) и заголовок «Настройки помодоро».

В standalone-приложении (`apps/horologion/`) topbar собран по другой схеме — через IPC `horologion:settings:open` открывается отдельное BrowserWindow (см. [Окно настроек](#окно-настроек) ниже). В extension'е settings — это просто роут внутри того же окна.

## Окно настроек

Settings — **отдельное Electron BrowserWindow** (не модалка, не route в основном окне). Кнопка ⚙ в тайтлбаре и «Настройки помодоро» внутри pomodoro-card зовут `window.horologion.settings.open()` → IPC `horologion:settings:open` → main создаёт второе окно 560×680 с тем же preload и hash `#/settings`. App.vue видит `route.path === '/settings'` и рендерит только SettingsView внутри `DesktopChrome + DesktopContentSurface` (единый стиль с main).

В Settings:
- Длительности (work / shortBreak / longBreak / pomodorosUntilLongBreak) — input[type=number] с 2px border, без spin-button'ов.
- Поведение — toggles (трекать break как «Отдых» / autostart work / autostart break / системные уведомления / режим стримера — отключает паузу рендеринга при перекрытии окна; toggle мирорится из renderer'а в `userData/horologion-settings.json` через IPC `horologion:streamerMode:set`, main применяет `disable-features=CalculateNativeWinOcclusion` + `disable-backgrounding-occluded-windows` ДО `app.whenReady`; на toggle в проде делаем `app.relaunch()`, в деве авто-рестарт пропускаем — `VITE_DEV_SERVER_URL` теряется при self-relaunch).
- Звуки — kosmos-visuals `Dropdown` (shadcn-стиль вместо native `<select>`) для выбора звука конца work / конца break + кнопка тестирования + slider громкости (`pomodoroSettings.ringtoneVolume`, sync'ится через `setVolumeMultiplier` в `lib/sounds.ts`).
- Reset — кнопка стиля `.pomo__secbtn` в destructive-цвете.

## Close-to-tray

- Закрытие окна не убивает приложение — окно прячется, Horologion живёт в системном трее.
- Tray-иконка с меню: «Открыть Horologion» / «Выйти». Клик по иконке = toggle show/hide.
- Pomodoro-таймер продолжает тикать в фоне (Vue renderer живёт).
- Реальный quit — только через tray «Выйти» (тогда `ArkClient.stop()` корректно останавливает sidecar).

::: warning Future
Сейчас pomodoro-состояние живёт в Vue renderer. Если Electron упадёт — состояние pomodoro потеряется (но активный `time_entry_obj` уже в ARK с `startedAt`). Долгосрочно — перенос pomodoro state machine в ark-core-rpc, чтобы переживать полный quit. Это TODO в roadmap.
:::

## Команды

```powershell
cd apps/horologion
bun run typecheck
bun run dev               # cargo build sidecar:dev + vite + Electron
bun run build:js          # release sidecar + tsc + vite build (без установщика)
bun run build             # build:js + electron-builder --win nsis (финальный NSIS one-click)
bun run package:dir       # unpacked desktop bundle
bun run test:e2e          # Playwright (.e2e/ изолированная БД)
```

::: tip Билд
По общей [конвенции Kosmos](/reference/commands#конвенция-сборки-релизов) `bun run build` собирает **NSIS one-click** установщик — `apps/horologion/release/Horologion Setup X.Y.Z.exe`. Ставится в `%LocalAppData%\Horologion` без UAC, без мастера (стиль Linear / Slack / Discord). `runAfterFinish: true` сразу запускает приложение после установки.

**Иконка** embed'ится в `Horologion.exe` через `afterPack`-хук (`build/afterPack.cjs`), использующий npm-пакеты `rcedit` + `png-to-ico`. Это нужно, потому что `win.signAndEditExecutable: false` отрубает встроенный rcedit electron-builder (workaround под падение winCodeSign symlinks на Windows без Developer Mode). Хук конвертирует `build/icon.png` → `build/icon.ico` (с кэшем по mtime), затем зовёт rcedit и проставляет иконку + version-string метаданные (ProductName, CompanyName, FileVersion). Дополнительно в main.ts вызывается `app.setAppUserModelId("com.kazui.horologion")`, чтобы Windows правильно группировал окно в taskbar и подхватывал нашу иконку, а не дефолтную electron.exe.

Итог: иконка отображается в окне, трее, taskbar, Start Menu, Проводнике.
:::

## Command bus integration

Horologion регистрируется в [Kepler command bus](/concepts/command-bus) как provider экшенов. Когда юзер открывает Kepler launcher (`Ctrl+Shift+K`) и выбирает horologion-команду — она роутится к нашей апке и реально что-то запускает.

### Зарегистрированные команды

| ID | Что делает |
|---|---|
| `horologion:pomodoro:25` | Запускает помодоро на 25 минут |
| `horologion:pomodoro:50` | Запускает помодоро на 50 минут |
| `horologion:stopwatch:start` | Запускает секундомер |

Регистрация — в `electron/main.ts` через `ArkClient.commands.register([...])` после того, как `arkClient` ready. Snipet:

```ts
await client.commands.register([
  { id: "horologion:pomodoro:25", title: "Pomodoro 25 минут", subtitle: "Horologion", category: "action" },
  { id: "horologion:pomodoro:50", title: "Pomodoro 50 минут", subtitle: "Horologion", category: "action" },
  { id: "horologion:stopwatch:start", title: "Запустить секундомер", subtitle: "Horologion", category: "action" },
]);
```

### IPC флоу: main → renderer

Main подписывается на backend событие `command_invoked` для своих id'ов. При получении формирует типизированный payload и отправляет в renderer через `webContents.send("horologion:cmd", payload)`:

```ts
type PomodoroStartPayload = { kind: "pomodoro:start"; durationMin: 25 | 50 };
type StopwatchStartPayload = { kind: "stopwatch:start" };
type HorologionCommandPayload = PomodoroStartPayload | StopwatchStartPayload;
```

Preload экспонирует подписку `window.horologion.onCommand((payload) => { ... })` через `ipcRenderer.on("horologion:cmd", ...)`.

### Renderer: App.vue dispatcher

`App.vue` при mount подписывается на `window.horologion.onCommand`:

- `kind === "pomodoro:start"` → `pomodoro.start({ workMinOverride: payload.durationMin })`.
- `kind === "stopwatch:start"` → `timeEntries.startTimer({ source: "stopwatch" })`.

### `workMinOverride` — per-session override

`usePomodoro` принимает опциональный `workMinOverride` в `start({ workMinOverride })`. Если задан — этот work-сегмент идёт на N минут, **не** мутируя `pomodoroSettings.workMin` (persistent). Следующая сессия (без override) вернётся к настройкам пользователя. Это нужно чтобы `horologion:pomodoro:50` запускал именно 50-минутный focus, не трогая дефолт (обычно 25).

## Связанные документы

- [Roadmap](/apps/horologion-roadmap) — что планируется / баги.
- [Command bus](/concepts/command-bus) — протокол dynamic commands.
- [Модель данных ARK](/concepts/ark-objects) — `time_entry_obj`, `tag_obj`.
- [Delphi](/apps/delphi) — задачи (для `@`-mention).
- [@kosmos/ark](/packages/kosmos-ark) — TS SDK.
- [kosmos-visuals](/packages/kosmos-visuals) — UI-система.
