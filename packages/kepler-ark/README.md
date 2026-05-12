# @kepler/ark

Node/Electron-main SDK for the Ark runtime.

This package talks to `ark-core-rpc`, the Rust sidecar in `packages/ark-core/rust`.
Renderers should not use this package directly. Expose a narrow preload API from
Electron main instead.

## Runtime Modes

### Self-managed sidecar

Use this mode when the caller wants `ArkClient` to spawn and own
`ark-core-rpc`.

```ts
import { ArkClient } from '@kepler/ark'

const ark = new ArkClient({
  spaceId: 'default',
  deviceId: 'device-1',
  deviceName: 'Workstation',
  authSecret: 'shared-space-secret',
  dbPath: 'C:/Users/me/AppData/Roaming/Kepler/spaces/default/ark.db',
  sidecarPath: 'C:/path/to/ark-core-rpc.exe',
})

await ark.start()
```

`dbPath` is required in this mode. The client sends `init` automatically before
the first sync, object, or usage request. Requests sent to self-managed sidecars
use request ids so responses can be matched even when sidecar output also
contains async events.

### Injected sidecar

Use this mode when another layer already owns the sidecar process, for example
an Electron `sidecar.ts` module shared by multiple app services.

```ts
const ark = new ArkClient({
  spaceId: 'default',
  deviceId: 'device-1',
  requestFn: sidecar.request,
  onEventFn: sidecar.onEvent,
})

await ark.start()
```

In injected mode, the owner of `requestFn` is responsible for initializing the
database and managing the binary lifecycle. `@kepler/ark` keeps request shapes
legacy-compatible and does not add request ids to injected calls.

## Sync API

```ts
await ark.start()

ark.onPeerConnected((deviceId, deviceName) => {
  console.log('peer connected', deviceId, deviceName)
})

ark.onEntityChanged((entityJson) => {
  console.log('entity changed', entityJson)
})

await ark.broadcastChange('object', 'obj-1', {
  type: 'object',
  id: 'obj-1',
  data: {},
  hlc: '0:0:device-1',
})

const peers = await ark.getConnectedPeers()
await ark.stop()
```

Relay options are forwarded to `ark-core-rpc`. When `relayUrl` is set, the
sidecar starts a relay bridge beside LAN sync and exchanges the same sync
frames through the relay server.

`authSecret` is optional. When it is set on every device in a space, LAN/P2P
sync authenticates `hello` messages with HMAC-SHA256. This prevents peers
without the shared secret from joining the mesh, but it does not encrypt traffic.

## Object API

```ts
const objects = await ark.objects.list()
const taskObjects = await ark.objects.listByType('task_obj')
const task = await ark.objects.get('task-1')
const linked = await ark.objects.getMany(['task-1', 'note-1'])

await ark.objects.upsert({
  id: 'task-1',
  typeId: 'task_obj',
  title: 'Draft plan',
  contentJson: { body: '' },
  propsJson: { status: 'open' },
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  deletedAt: null,
})

const matches = await ark.objects.search('plan')
await ark.objects.delete('task-1')
```

`listByType` and `getMany` are real `ark-core-rpc` query operations, not
SDK-side filtering over `list()`.

## Object Types And Links

```ts
await ark.objectTypes.upsert({
  id: 'task_obj',
  name: 'Task',
  schemaJson: '{}',
  uiSchemaJson: '{}',
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  systemLocked: false,
})

await ark.links.upsert({
  id: 'link-1',
  sourceObjectId: 'task-1',
  targetObjectId: 'note-1',
  linkType: 'related',
  createdAt: new Date().toISOString(),
})
```

## Usage API

```ts
const usage = await ark.usage.loadAll()
const recentProcesses = await ark.usage.processes.recent(10)
const matchingProcesses = await ark.usage.processes.search('demo', 10)
const gamePlaytime = await ark.usage.gamePlaytime.summary({
  bindings: [
    {
      gameId: 'game-1',
      gameName: 'Demo',
      matchType: 'exe_path',
      matchValue: 'C:/Games/Demo/demo.exe',
    },
  ],
  rangeStart: '2026-04-01',
  rangeEnd: '2026-04-30',
})

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
})

await ark.usage.sessions.delete('session-1')
await ark.usage.events.delete('event-1')
```

Usage process and game playtime summary queries are backed by Rust/SQLite
aggregation in `ark-core-rpc`; callers pass app-specific bindings, not SQL.

## Integration Rules

- Electron renderer code should call a preload facade, not `ArkClient`.
- Electron main services should call this SDK instead of opening Ark SQLite
  directly.
- Rust services may use `ark_core::db` directly when they update Ark's sync
  state atomically through the core helpers.
- Direct SQL writes from application code bypass validation, event delivery,
  typed errors, and future sync journal behavior.
