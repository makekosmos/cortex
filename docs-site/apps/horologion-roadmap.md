# Horologion — Roadmap

Список фич/идей/багов для Horologion. Поддерживается вручную (правь напрямую). Снято с разговоров с пользователем. URL страницы и workspace-директория остались `horologion` — это исторический code-name, см. [Horologion → Имя](/apps/horologion#имя).

## Ближайшее

Фичи в работе или которые хочется сделать **скоро**. По приоритету ↓.

### Focus view

Когда работает фокус-фаза (work-сегмент pomodoro) — отдельный режим / экран:

- Что сейчас в фокусе (description + linked task).
- Крупно — идущее время.
- Лёгкая анимация (пульс / прогресс).
- Минимум отвлекающих элементов, можно по hotkey'у вызвать поверх остального.

### Фоновые звуки во время фокуса

Опциональный ambient-фон во время work-сегмента: дождь / коричневый шум / огонь / etc. Запускается на старте work, останавливается на конце фазы (или вручную). Громкость регулируемая. Звуки храним локально, lightweight loop.

### Календарный вид

Вкладка / экран — посмотреть «сегодня в какие промежутки чем занимался»:

- День разбит на часы (вертикальная ось).
- Блоки записей нарисованы по высоте = длительность.
- Возможность пролистывать назад / вперёд по дням / неделям.
- Клик на блок → открыть Edit modal.

## Потом

То что хочется, но не критично — пока в idea-bin.

- **Tag picker** в input bar и Edit modal — UI для `tag_obj`, общий с Delphi.
- **Real `object_link`** между `time_entry_obj` и `task_obj` (сейчас только `propsJson.taskId`, не настоящая связь ARK).
- **Reports / charts** — за неделю / месяц, по тегам / задачам.
- **Импорт из usage-tracker** — предложить time_entry из активного периода с предложением «ты работал тут в Chrome 28 мин, было это «X»?».
- **Hotkeys** — глобальные клавиши для start/stop, pomodoro skip, и т.д.
- **Calendar week-strip в EditModal** — добавить возможность типа «вчера / 2 дня назад» вместо месячной навигации.
- **Soft delete / корзина** — сейчас delete = permanent.
- **Чип-пикер задачи** в input row (`📁` иконка) — выбор задачи без `@`.
- **Goo / metaballs соединение** draft input ↔ timer card при активной сессии. Пробовали через SVG-filter — текст в card'ах блёрился из-за `feGaussianBlur`. Для production-quality нужна двухслойная архитектура: background-only-слой с goo + content-слой без filter. Сейчас вместо этого простое соединение: gap → 0 + плавное выпрямление прилегающих углов.

## Сделано

- **Pomodoro state в kepler-backend** (2026-05-22). `services/kepler-backend/src/pomodoro_host.rs` (`PomodoroHost`) держит state machine, эмитит `pomodoro_tick` / `pomodoro_phase_changed` / `pomodoro_finished` через ARK event bus. Horologion-окно можно закрывать — таймер тикает в фоне, focus widget и notifications продолжают работать. Полный quit Kepler shell всё ещё останавливает таймер (sidecar убивается вместе с shell'ом), но это уже корректно: «сегмент закрыт» с правильным `endedAt`.
- **`keepAliveInBackground` для Horologion-окна** (2026-05-22). Manifest имеет флаг, shell intercept'ит close → hide, renderer переживает закрытие.

## Баги / замечания

Открытые проблемы, на которые надо вернуться.

- ~~HomeView должен читать `query.mode` при mount.~~ **Сделано (2026-05-16).** HomeView watch'ит `route.query.mode` с `immediate: true` → переключает `timerMode` при cold open и при reuse уже открытого окна. Route доставляется через IPC (`kepler.navigation.onNavigate`) — см. [Extension host → Deep links](/concepts/extension-host#deep-links-через-route). Reuse кейс тоже работает: повторный invoke «Помодоро» при открытом Horologion переключит таб live.
- **Sync port 21531 коллизит** между любыми двумя ARK sidecar'ами (Delphi + Horologion, или установленный Horologion + dev Horologion, и т.д.). Второй sidecar падает с «Failed to bind 0.0.0.0:21531» (os error 10048). CRUD продолжает работать, но в трейле горит красный dot + toast с ошибкой при старте. **Воспроизводится регулярно** — особенно когда установленный из MSI Horologion сидит в трее, а юзер запускает dev. Долгосрочный фикс: либо port-discovery (range 21531-21540 с retry на bind), либо разделение port'ов по `appId`, либо вообще централизованный sync-broker. Пока в качестве workaround — убивать «лишний» инстанс перед запуском нужного.
- **E2E (`test:e2e`) падает** если параллельно крутится dev-сервер любого приложения с ARK sidecar — тот же port 21531. Тесты не изолируют sync-порт.

## Как обновлять

Просто правь этот файл руками. Если фича в работе — оставь её в «Ближайшее» с пометкой статуса (например `WIP — scaffold`, `WIP — UI готов, нет IPC`). Когда сделана — убирай отсюда и фиксируй в [обзоре Horologion](/apps/horologion).

## Связанные документы

- [Обзор Horologion](/apps/horologion).
- [Roadmap-конвенция](/agents/docs-maintenance#roadmap) — как такие страницы устроены для всех приложений.
