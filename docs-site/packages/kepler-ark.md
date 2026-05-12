# @kepler/ark — TypeScript SDK

::: tip Источник правды
`packages/kepler-ark/README.md`
:::

`@kepler/ark` — канонический TypeScript-клиент для ARK runtime. Используется в Electron main и в Node-сервисах. Renderer'ы **не** используют его напрямую — для renderer'ов поднимается узкий preload API в Electron main.

## Установка

Пакет workspace-локальный, добавляется в `dependencies` приложения:

```json
{
  "dependencies": {
    "@kepler/ark": "workspace:*"
  }
}
```

## Режимы работы

### Self-managed sidecar

`ArkClient` спавнит и владеет процессом `ark-core-rpc.exe`. Используй когда приложение — единственный потребитель ARK.

```ts
import { ArkClient } from '@kepler/ark';

const ark = new ArkClient({
  spaceId: 'default',
  deviceId: 'device-1',
  deviceName: 'Workstation',
  authSecret: 'shared-space-secret',
  dbPath: 'C:/Users/me/AppData/Roaming/Kepler/spaces/default/ark.db',
  sidecarPath: 'C:/path/to/ark-core-rpc.exe',
});

await ark.start();
```

`dbPath` обязателен. Клиент шлёт `init` автоматически перед первым sync/object/usage запросом. Запросы к self-managed sidecar используют request id, чтобы матчить ответы даже когда в stdout есть async events.

### Injected sidecar

Sidecar уже владеется другим слоем (например, `apps/delphi/ts/electron/sidecar.ts`, который делит sidecar между несколькими сервисами). `ArkClient` получает `requestFn` и `onEventFn`.

```ts
const ark = new ArkClient({
  spaceId: 'default',
  deviceId: 'device-1',
  requestFn: sidecar.request,
  onEventFn: sidecar.onEvent,
});

await ark.start();
```

В injected mode владелец `requestFn` отвечает за инициализацию БД и жизненный цикл бинаря. `@kepler/ark` сохраняет legacy-совместимый формат запросов и **не** добавляет request id'ы в injected calls.

## Sync API

```ts
await ark.start();

ark.onPeerConnected((deviceId, deviceName) => {
  console.log('peer connected', deviceId, deviceName);
});

ark.onEntityChanged((entityJson) => {
  console.log('entity changed', entityJson);
});

await ark.broadcastChange('object', 'obj-1', {
  type: 'object',
  id: 'obj-1',
  data: { /* ... */ },
  hlc: '0:0:device-1',
});

const peers = await ark.getConnectedPeers();
await ark.stop();
```

Relay options пробрасываются в `ark-core-rpc`. Когда `relayUrl` установлен, sidecar поднимает relay-bridge рядом с LAN sync.

`authSecret` опционален. Когда установлен на каждом устройстве space'а, LAN/P2P аутентифицирует `hello`-сообщения HMAC-SHA256.

## Object API

```ts
const objects = await ark.objects.list();
const tasks = await ark.objects.listByType('task_obj');
const task = await ark.objects.get('task-1');
const linked = await ark.objects.getMany(['task-1', 'note-1']);

await ark.objects.upsert({
  id: 'task-1',
  typeId: 'task_obj',
  title: 'Draft plan',
  contentJson: { body: '' },
  propsJson: { status: 'open' },
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  deletedAt: null,
});

const matches = await ark.objects.search('plan');
await ark.objects.delete('task-1');
```

`listByType` и `getMany` — реальные `ark-core-rpc` query-операции, **не** SDK-side фильтрация над `list()`.

## Object Types и Links

```ts
await ark.objectTypes.upsert({
  id: 'task_obj',
  name: 'Task',
  schemaJson: '{}',
  uiSchemaJson: '{}',
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  systemLocked: false,
});

await ark.links.upsert({
  id: 'link-1',
  sourceObjectId: 'task-1',
  targetObjectId: 'note-1',
  linkType: 'related',
  createdAt: new Date().toISOString(),
});
```

## Usage API

```ts
const usage = await ark.usage.loadAll();
const recent = await ark.usage.processes.recent(10);
const found = await ark.usage.processes.search('demo', 10);

const summary = await ark.usage.gamePlaytime.summary({
  bindings: [{
    gameId: 'game-1',
    gameName: 'Demo',
    matchType: 'exe_path',
    matchValue: 'C:/Games/Demo/demo.exe',
  }],
  rangeStart: '2026-04-01',
  rangeEnd: '2026-04-30',
});

await ark.usage.trackedApps.upsert({
  id: 'app-1',
  platform: 'windows',
  exePath: 'C:/Games/Demo/demo.exe',
  normalizedExePath: 'c:/games/demo/demo.exe',
  processName: 'demo.exe',
  displayName: 'Demo',
  publisher: null,
  iconRef: null,
  firstSeenAt: new Date().toISOString(),
  lastSeenAt: new Date().toISOString(),
});

await ark.usage.sessions.delete('session-1');
await ark.usage.events.delete('event-1');
```

Usage process и game playtime summary — Rust/SQLite агрегация в `ark-core-rpc`. Caller передаёт app-specific bindings, не SQL.

## Selected-space helpers

Identity-слой для shared selected space (`appData/Kepler/selected-space.json`). Используется всеми Electron-приложениями, чтобы Eden / Delphi / Arrancador резолвили один и тот же ARK DB path.

```ts
import {
  buildPersonalSelectedSpace,
  buildSharedSelectedSpaceFromCode,
  readSharedSelectedSpace,
  writeSharedSelectedSpace,
  getArkDbPathForSelectedSpace,
  getKeplerDataDir,
  derivePersonalSpaceCodeFromVaultPath,
  deriveSpaceIdFromCode,
  type SharedSelectedSpace,
} from '@kepler/ark';

const selection = readSharedSelectedSpace(app.getPath('appData'));
const dbPath = getArkDbPathForSelectedSpace(app.getPath('appData'), selection);
```

`spaceCode` — Crockford-base32 от хэша нормализованного vault path (12 символов). `spaceId` — первые 16 hex-символов SHA-256 от нормализованного `spaceCode`.

## Правила интеграции

::: danger
- **Renderer** — preload facade, **не** `ArkClient`.
- **Electron main services** — этот SDK, **не** прямое открытие ARK SQLite.
- **Rust services** — `ark_core::db` напрямую (через core helpers, обновляющие sync state).
- Прямые SQL writes из app-кода обходят validation, event delivery, typed errors и будущий sync journal.
:::

## Связанные документы

- [ark-core](/packages/ark-core) — runtime.
- [Граница записи в ARK](/concepts/write-boundary).
- [Модель данных ARK](/concepts/ark-objects).
- [Синхронизация](/concepts/sync).
