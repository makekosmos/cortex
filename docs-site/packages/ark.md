# @kosmos/ark — TypeScript SDK

::: tip Источник правды
`packages/ark/README.md`
:::

`@kosmos/ark` — канонический TypeScript-клиент для ARK runtime. Используется в Electron main и в Node-сервисах. Renderer'ы **не** используют его напрямую — для renderer'ов поднимается узкий preload API в Electron main.

## Установка

Пакет workspace-локальный, добавляется в `dependencies` приложения:

```json
{
  "dependencies": {
    "@kosmos/ark": "workspace:*"
  }
}
```

## Режимы работы

### Self-managed sidecar

`ArkClient` спавнит и владеет процессом `ark-core-rpc.exe`. Legacy режим для апок вне Kepler-экосистемы.

```ts
import { ArkClient } from '@kosmos/ark';

const ark = new ArkClient({
  spaceId: 'default',
  deviceId: 'device-1',
  dbPath: 'C:/Users/me/AppData/Roaming/Kosmos/spaces/default/ark.db',
  sidecarPath: 'C:/path/to/ark-core-rpc.exe',
});
await ark.start();
```

`dbPath` обязателен. Клиент шлёт `init` автоматически перед первым sync/object/usage запросом. Запросы используют request id, чтобы матчить ответы даже когда в stdout async events.

### Injected sidecar

Sidecar уже владеется другим слоем. `ArkClient` получает `requestFn` и `onEventFn`. В injected mode владелец отвечает за инициализацию БД и жизненный цикл бинаря; `@kosmos/ark` сохраняет legacy-совместимый формат запросов и **не** добавляет request id'ы.

```ts
const ark = new ArkClient({ spaceId: 'default', deviceId: 'device-1', requestFn: sidecar.request, onEventFn: sidecar.onEvent });
await ark.start();
```

### Kepler mode (Phase 2+)

Когда на машине запущен [Kepler host](../apps/kepler.md), `ArkClient` коннектится к нему через локальный WebSocket вместо spawn'а собственного `ark-core-rpc`. Это даёт single sidecar на машине.

```ts
import { ArkClient, ensureKeplerRunning } from '@kosmos/ark';

const state = await ensureKeplerRunning({
  appDataPath: app.getPath('appData'),
  waitMs: 10000,
  autoLaunch: true,  // если Kepler exe найден, но не запущен — стартуем
});

if (state.kind === 'connected') {
  const ark = new ArkClient({
    spaceId: 'default',
    deviceId: 'device-1',
    keplerLock: state.lock,  // ← включает kepler mode
  });
  await ark.start();
}
```

`ensureKeplerRunning()` возвращает `KeplerState` (discriminated union):

| `kind` | Когда | Что делать |
|---|---|---|
| `connected` | lock-file есть, PID жив, MAJOR матчит | использовать `state.lock` в `keplerLock` |
| `not-installed` | Kepler exe не найден в conventional paths | показать modal «Скачать Kepler» (Phase 6) или fallback (transitional) |
| `launch-failed` | exe найден, spawn'нулся, но lock-file не появился за `waitMs` | toast «Kepler не отвечает», fallback |
| `incompatible-version` | MAJOR mismatch между Kepler и клиентом | modal «Update Kepler/app» |

В kepler mode `ArkClient` **не** вызывает `init` (Kepler уже init'нул ARK) и **не** вызывает `start_sync` (Kepler владеет LAN sync — Phase 5 централизация).

### Wire-формат kepler mode

Тот же JSON-RPC что self-managed, плюс hello-handshake (см. [kepler.md](../apps/kepler.md#protocol-hello-handshake)). После handshake:

```json
// Request:  { "operation": "<name>", "_req_id": "...", ...params }
// Response: { "_req_id": "...", "ok": true, "data": {...} }
// Event:    { "event": "<kind>", ... }  // нет _req_id, нет ok
```

Известные `event.kind`: `entity_changed`, `peer_connected`, `peer_disconnected`, `command_invoked`, `commands_changed`. Диспатч событий — общий `dispatchSidecarEvent`, работает идентично в self-managed (stdout-кадры) и kepler mode (WS-кадры).

### `invokeOperation` escape-hatch

Legacy callsites используют 30+ `runArkRequest({operation: ..., ...})`. Чтобы не переписывать всё одновременно с cutover'ом, `ArkClient` имеет public `invokeOperation<T>({ operation, ... })`. Постепенная миграция на typed API (`ark.objects.list()`) — отдельная follow-up задача.

## Sync API

```ts
await ark.start();
ark.onPeerConnected((deviceId, deviceName) => { /* ... */ });
ark.onEntityChanged((entityJson) => { /* ... */ });
await ark.broadcastChange('object', 'obj-1', { type: 'object', id: 'obj-1', data: {}, hlc: '0:0:device-1' });
const peers = await ark.getConnectedPeers();
await ark.stop();
```

Relay options пробрасываются в `ark-core-rpc`. Когда `relayUrl` установлен, sidecar поднимает relay-bridge рядом с LAN sync.

`authSecret` опционален. Когда установлен на каждом устройстве space'а, LAN/P2P аутентифицирует `hello`-сообщения HMAC-SHA256.

## Commands API

`commands` — namespace для регистрации «ручек» приложений в Kepler launcher. Каждая апка публикует свой список команд (например, «Pomodoro 25 минут», «Создать заметку», «Открыть Delphi»), Kepler-launcher агрегирует их и показывает в command-palette. При выборе команды Kepler шлёт `command_invoked` обратно — апка выполняет действие.

Работает в обоих режимах транспорта; wire-формат см. [Command Bus](/concepts/command-bus).

### API

```ts
interface CommandManifest {
  id: string;             // глобально уникальный, конвенция "<app>:<feature>:<verb>"
  title: string;          // отображается в palette
  subtitle?: string;      // обычно имя апки или контекст
  category: 'open' | 'action';
}

interface CommandInvokedEvent {
  id: string;
  params?: Record<string, unknown>;
  invokerClientId?: string; // кто инициировал invoke
}

interface ArkCommandsApi {
  register(commands: CommandManifest[]): Promise<void>;
  unregister(ids: string[]): Promise<void>;
  list(): Promise<CommandManifest[]>;
  invoke(id: string, params?: Record<string, unknown>): Promise<void>;
  onInvoked(handler: (event: CommandInvokedEvent) => void): () => void;
  onChanged(handler: (commands: CommandManifest[]) => void): () => void;
}
```

### Пример: app-side (провайдер команд)

```ts
import { ArkClient } from '@kosmos/ark';

await ark.commands.register([
  {
    id: 'horologion:pomodoro:25',
    title: 'Pomodoro 25 минут',
    subtitle: 'Horologion',
    category: 'action',
  },
]);

const off = ark.commands.onInvoked((event) => {
  if (event.id === 'horologion:pomodoro:25') {
    startPomodoro(25);
  }
});

// На shutdown:
off();
await ark.commands.unregister(['horologion:pomodoro:25']);
```

### Пример: launcher-side

```ts
const cmds = await ark.commands.list();          // отрендерить в palette
await ark.commands.invoke('horologion:pomodoro:25'); // провайдер получит CommandInvokedEvent
const offChanged = ark.commands.onChanged((next) => rerenderPalette(next));
```

`onInvoked` / `onChanged` возвращают unsubscribe-функцию. Подписки переживают переподключение к kepler-host (callback'и хранятся в `ArkClient` instance).

## Object API

```ts
const objects = await ark.objects.list();
const tasks = await ark.objects.listByType('task_obj');
const task = await ark.objects.get('task-1');
const linked = await ark.objects.getMany(['task-1', 'note-1']);
const matches = await ark.objects.search('plan');
await ark.objects.upsert({ id: 'task-1', typeId: 'task_obj', title: 'Draft plan', contentJson: {}, propsJson: {}, createdAt: '...', updatedAt: '...', deletedAt: null });
await ark.objects.delete('task-1');
```

`listByType` и `getMany` — реальные `ark-core-rpc` query-операции, **не** SDK-side фильтрация над `list()`.

## Object Types и Links

```ts
await ark.objectTypes.upsert({ id: 'task_obj', name: 'Task', /* ... */ });
await ark.links.upsert({ id: 'link-1', sourceObjectId: 'task-1', targetObjectId: 'note-1', linkType: 'related', createdAt: '...' });
```

## Usage API

```ts
const usage = await ark.usage.loadAll();
const recent = await ark.usage.processes.recent(10);
const summary = await ark.usage.gamePlaytime.summary({ bindings, rangeStart, rangeEnd });
await ark.usage.trackedApps.upsert({ /* ... */ });
await ark.usage.sessions.delete('session-1');
```

Usage process и game playtime summary — Rust/SQLite агрегация в `ark-core-rpc`. Caller передаёт app-specific bindings, не SQL.

## Правила интеграции

::: danger
- **Renderer** — preload facade, **не** `ArkClient`.
- **Electron main services** — этот SDK, **не** прямое открытие ARK SQLite.
- **Rust services** — `ark_core::db` напрямую (через core helpers, обновляющие sync state).
- Прямые SQL writes из app-кода обходят validation, event delivery, typed errors и будущий sync journal.
:::

## Связанные документы

- [ark-core](/packages/ark-core) — runtime.
- [Kepler host](/apps/kepler) — single-sidecar host и launcher.
- [Command Bus](/concepts/command-bus) — wire-формат `commands.*` и событий.
- [Граница записи в ARK](/concepts/write-boundary).
- [Модель данных ARK](/concepts/ark-objects).
- [Синхронизация](/concepts/sync).

## Источники

- `packages/ark/src/ark-client.ts` — `ArkClient`, `ArkCommandsApi`, dispatch событий.
- `packages/ark/src/ensure-kepler.ts` — `ensureKeplerRunning`, discovery + auto-launch.
