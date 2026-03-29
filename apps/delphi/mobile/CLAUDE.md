# Delphi Mobile

GTD-менеджер задач для iOS/Android. Часть экосистемы Kosmos.

## Стек

- **Expo** (SDK 55) + **React Native** 0.83
- **expo-router** — файловый роутинг
- **Zustand** — стейт-менеджмент
- **expo-sqlite** — локальное хранилище
- **expo-crypto** — UUID генерация

## Структура

```
app/
  _layout.tsx          # Root layout, инициализация Ark sync
  (tabs)/
    _layout.tsx        # Tab navigator (4 вкладки)
    index.tsx          # Сегодня (дефолт на мобильных)
    inbox.tsx          # Входящие
    upcoming.tsx       # Планы
    logbook.tsx        # Журнал
  settings.tsx         # Настройки (модальное окно)
src/
  types/task.ts        # Модели данных (общие с ts/)
  models/              # Фабрики и трансформации
  store/todos.ts       # Zustand store с SQLite персистентностью
  services/
    sync/ark-client.ts # WebSocket sync с Ark
    sync/hlc.ts        # Hybrid Logical Clock
    sync/pairing.ts    # Парсинг ark:// строк
    filters/           # Фильтрация по smart lists
  components/
    TodoRow.tsx         # Строка задачи
    SmartListScreen.tsx # Переиспользуемый экран списка
  hooks/
    useSmartList.ts     # Хук фильтрации
  db/
    storage.ts          # expo-sqlite обёртка
```

## Навигация

Дефолтный экран — **Сегодня** (не Входящие, как на десктопе).

4 вкладки: Сегодня, Входящие, Планы, Журнал.

Настройки — через иконку в хедере, открывается как модальное окно.

## Синхронизация

Подключение к Ark через WebSocket (`ws://ark-server/ws/sync?key=API_KEY`).

Строка подключения: `ark://host:port?key=SECRET` — вводится в настройках или сканируется QR.

HTTP fetch (`GET /events?event_type=task&limit=1000`) загружает существующие задачи при подключении.

Incoming данные мержатся через `upsertTodos` (не перезаписывают локальные задачи).

Localhost/127.0.0.1 URL в настройках автоматически удаляются (мобильный не может достучаться до localhost десктопа).

P2P mesh пока не реализован (только Ark relay).

## Metro (монорепо)

`metro.config.js` настроен для bun workspace:
- `projectRoot` = mobile app dir
- `watchFolders` = monorepo root (для hoisted deps)
- `nodeModulesPaths` = local + root node_modules
- Custom `resolveRequest` — предотвращает HMR crash при resolve монорепо корня
- `scripts/fix-jsc-safe-url.js` — патч jsc-safe-url для пустых URL путей

## Команды

```bash
bun install
bun start              # Expo dev server
bun run ios            # iOS simulator
bun run android        # Android emulator
bunx tsc --noEmit      # Typecheck
```

## Conventions

- Язык UI: русский
- Тема: тёмная
- Package manager: bun
- Path alias: `@/` -> `src/`
