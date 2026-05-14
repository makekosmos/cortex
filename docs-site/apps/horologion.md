# Horologion — трекер времени

::: tip Статус
**MVP работает.** Единая Home-страница: pomodoro/секундомер с переключателем в шайбе (sliding pill), плавная анимация смены режимов (swipe + height transition card'а), @-mention задач Delphi (в т.ч. мульти-задачи в помодоро с равномерным дроблением сегмента), live duration в списке, edit-modal, ПКМ-удаление, группировка одинаковых, collapse/expand дней, помодоро со звуками и системными уведомлениями, tray + close-to-tray, NSIS one-click установщик, embed'нутая иконка в `.exe`.
:::

::: info Имя
Имя приложения — **Horologion** (греч. ὡρολόγιον — «часослов»). После Phase B-D Horologion живёт как Vue-extension внутри Kepler shell — `extensions/horologion/`. Внутренние идентификаторы (`HorologionApi`, `window.horologion`, IPC `horologion:*`) сохранились для совместимости с существующим Vue-кодом, через shim поверх Kepler ark bridge.
:::

- **Path**: `extensions/horologion/`
- **Стек**: Vue 3.6 Vapor + `@kepler/ark` + `@kepler/visuals`. Открывается через Kepler shell `extension-host.ts` в отдельном `BrowserWindow`.
- **Аналог**: Toggl Track — без социалки, без web-app, локально, с интеграцией Delphi-задач.

## Список фич, которые планируется/нужно сделать

См. [Roadmap](/apps/horologion-roadmap).

## Структура

```
extensions/horologion/
├─ manifest.json           # id, title, devPort, capabilities
├─ index.html
├─ vite.config.mjs
├─ icon.png                # отображается в Kepler launcher
└─ src/
   ├─ App.vue              # DesktopChrome + RouterView + scroll fade. На /settings рендерит только SettingsView
   ├─ main.ts              # Vue createApp + Inter Variable import + router mount
   ├─ router.ts            # `/` → HomeView, `/settings` → SettingsView (внутри memory router'а extension'а)
   ├─ styles.css           # --horologion-accent + локальные токены
   ├─ views/
   │  ├─ HomeView.vue      # Draft input + Pomodoro/Stopwatch toggle + список
   │  ├─ PomodoroView.vue  # Ring + ticks + dots + actions; читает usePomodoro
   │  ├─ StopwatchView.vue # Большое HH:MM:SS + primary-кнопка
   │  ├─ ListView.vue      # Группы по дням, collapse/expand, live duration tick
   │  └─ SettingsView.vue  # Группы настроек
   ├─ components/
   │  ├─ PomodoroDraftInput.vue  # Поле «Над чем работаем?» с chip'ами задач
   │  ├─ MentionInput.vue        # Generic @-mention обёртка
   │  ├─ MentionMenu.vue         # Popover автокомплита задач
   │  └─ EditEntryModal.vue      # Modal редактирования time_entry
   └─ lib/
      ├─ store.ts          # entriesChangedAt signal, pomodoroDraft, tasks cache, timerMode
      ├─ usePomodoro.ts    # state machine pomodoro (singleton)
      ├─ pomodoroSettings.ts  # настройки помодоро (localStorage)
      ├─ sounds.ts         # звуки конца work/break
      └─ format.ts         # formatDuration / dayKey / formatDayHeader
```

Запускается из Kepler launcher'а через команду `horologion:open` (открывает extension window). Иконка embed в shell.exe — за это отвечает `shell/build/`.

## UI и дизайн

Horologion полностью использует [`@kepler/visuals`](/packages/visuals): `<DesktopChrome>` + `<DesktopContentSurface>` обёртка, все цвета / шрифты / радиусы — только через CSS-переменные `@kepler/visuals`. **Никакого hardcoded `#hex` или собственного titlebar-кода.**

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
- **Справа**: круглый dot подключения к ARK (`var(--status-success)` / warning / `var(--destructive)`) + ⚙ Настройки (route `/settings` внутри extension window'а).
- Windows-controls справа — через `DesktopChrome` (Kepler shell сам управляет рамкой extension window'а).

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
- `<Modal>` из `@kepler/visuals`.
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

## Topbar

В Horologion-extension'е topbar содержит два UI-элемента справа от заголовка:

- **Status dot** — круглая точка (8px), цвет показывает состояние подключения к `kepler-backend`. Логика в `extensions/horologion/src/App.vue`: при mount и каждые 10 секунд дёргает дешёвую операцию `kepler.ark.request("list_object_types")` — успех → `connected` (зелёный), ошибка → `error` (красный), стартовое состояние → `connecting`. Tooltip переключается между «ARK подключен» / «Подключение к ARK…» / «ARK недоступен». Визуально совпадает с Delphi extension status dot (одни и те же oklch-токены из `@kepler/visuals`).
- **Кнопка ⚙ Настройки** — `router.push("/settings")` в memory-router'е extension'а. На route `/settings` App.vue прячет dot и кнопку, показывает «Назад» (`router.push("/")`) и заголовок «Настройки помодоро».

## Настройки

В Settings:
- Длительности (work / shortBreak / longBreak / pomodorosUntilLongBreak) — input[type=number] с 2px border, без spin-button'ов.
- Поведение — toggles (трекать break как «Отдых» / autostart work / autostart break / системные уведомления / режим стримера — отключает паузу рендеринга при перекрытии окна; toggle мирорится из renderer'а в `userData/horologion-settings.json` через IPC `horologion:streamerMode:set`, main применяет `disable-features=CalculateNativeWinOcclusion` + `disable-backgrounding-occluded-windows` ДО `app.whenReady`; на toggle в проде делаем `app.relaunch()`, в деве авто-рестарт пропускаем — `VITE_DEV_SERVER_URL` теряется при self-relaunch).
- Звуки — `@kepler/visuals` `Dropdown` (shadcn-стиль вместо native `<select>`) для выбора звука конца work / конца break + кнопка тестирования + slider громкости (`pomodoroSettings.ringtoneVolume`, sync'ится через `setVolumeMultiplier` в `lib/sounds.ts`).
- Reset — кнопка стиля `.pomo__secbtn` в destructive-цвете.

## Persistence pomodoro

::: warning Future
Сейчас pomodoro-состояние живёт в Vue renderer (extension). Если Kepler shell закрыт — pomodoro теряет таймер, но активный `time_entry_obj` уже записан в ARK с `startedAt`. Долгосрочно — перенос pomodoro state machine в ark-core-rpc либо в kepler-backend, чтобы переживать полный quit. Это TODO в roadmap.
:::

## Команды

Сборка проходит через Kepler shell:

```powershell
bun run --cwd shell build:extensions
bun run --cwd shell build:js
bun run --cwd shell dev
bun run --cwd shell test:e2e
```

В dev mode (`KEPLER_DEV=1`) extension поднимается с HMR (см. [Extension dev mode](/concepts/extension-dev-mode)).

Иконка в Kepler launcher отображается из `extensions/horologion/icon.png`.

## Command bus integration

Horologion регистрируется в [Kepler command bus](/concepts/command-bus) как provider экшенов. Когда юзер открывает Kepler launcher (`Ctrl+Shift+K`) и выбирает horologion-команду — она роутится к extension'у и реально что-то запускает.

### Зарегистрированные команды

| ID | Что делает |
|---|---|
| `horologion:pomodoro:25` | Запускает помодоро на 25 минут |
| `horologion:pomodoro:50` | Запускает помодоро на 50 минут |
| `horologion:stopwatch:start` | Запускает секундомер |

Регистрация — в extension main (`extensions/horologion/src/main.ts`) через `ArkClient.commands.register([...])` после подключения к `kepler-backend`.

Когда invoke приходит, App.vue dispatcher разводит:

- `horologion:pomodoro:25` / `:50` → `pomodoro.start({ workMinOverride: 25 | 50 })`.
- `horologion:stopwatch:start` → `timeEntries.startTimer({ source: "stopwatch" })`.

### `workMinOverride` — per-session override

`usePomodoro` принимает опциональный `workMinOverride` в `start({ workMinOverride })`. Если задан — этот work-сегмент идёт на N минут, **не** мутируя `pomodoroSettings.workMin` (persistent). Следующая сессия (без override) вернётся к настройкам пользователя. Это нужно чтобы `horologion:pomodoro:50` запускал именно 50-минутный focus, не трогая дефолт (обычно 25).

## Связанные документы

- [Roadmap](/apps/horologion-roadmap) — что планируется / баги.
- [Command bus](/concepts/command-bus) — протокол dynamic commands.
- [Модель данных ARK](/concepts/ark-objects) — `time_entry_obj`, `tag_obj`.
- [Delphi](/apps/delphi) — задачи (для `@`-mention).
- [@kepler/ark](/packages/ark) — TS SDK.
- [@kepler/visuals](/packages/visuals) — UI-система.
