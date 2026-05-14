# Extension host

Extension host — foundation для Phase 4: миграция продуктовых апок из отдельных standalone Electron .exe в **Vue extension bundles**, загружаемые внутри kepler-shell в отдельных BrowserWindow.

::: warning Текущий статус
Готов только loader (`apps/kepler-shell/electron/extension-host.ts`) + PoC manifest для Dashboard placeholder. **Реальные апки ещё не мигрированы**, остаются standalone .exe. Эта страница описывает foundation и план миграции, а не текущее production-состояние.
:::

## Цель

Сейчас каждая Kosmos-апка — отдельная .exe (Eden.exe, Delphi.exe, Horologion.exe, Arrancador.exe, Dashboard.exe). Это значит:

- Память: каждая Electron .exe = ~120-200 MB resident. Пять апок открытых одновременно = ~1 GB только под shell'ы.
- Update: пять отдельных NSIS installer'ов / GitHub Releases.
- Onboarding: пользователь должен установить и запустить каждую апку отдельно.

Phase 4 цель — превратить апки в **extensions**: Vue-бандл + manifest, который загружается kepler-shell'ом в новый BrowserWindow по требованию. Один Electron host на машину (kepler.exe), N extension windows. Update'ы — через kepler.

## Текущая реализация loader'а

`apps/kepler-shell/electron/extension-host.ts`:

```ts
export interface ExtensionManifest {
  id: string
  name: string
  entryHtml: string
  preload?: string          // optional, per-extension preload bundle
  width?: number            // default 900
  height?: number           // default 600
}

export function loadExtensionManifest(id: string): ExtensionManifest | null
export function listExtensions(): ExtensionManifest[]
export function openExtension(id: string): void  // reuse if already open, иначе create
```

Resolver:

```ts
function resolveExtensionsRoot(): string {
  // dev:  <repo>/apps/kepler-shell/extensions/
  // prod: process.resourcesPath/extensions/
  const dev = path.resolve(__dirname, '..', 'extensions')
  if (existsSync(dev)) return dev
  return path.join(process.resourcesPath ?? __dirname, 'extensions')
}
```

Reuse: `Map<id, BrowserWindow>`. Если окно уже открыто — `focus()`. На `closed` — `delete` из map.

### Manifest format

```json
{
  "id": "dashboard",
  "name": "Dashboard (PoC)",
  "entryHtml": "index.html",
  "preload": "preload.js",
  "width": 720,
  "height": 480
}
```

### Структура extension директории

```text
apps/kepler-shell/extensions/<id>/
├── manifest.json
├── index.html           // entryHtml — корень Vue bundle
├── bundle.js            // транспилированный Vue
├── style.css
└── preload.js           // optional, Electron preload bridge
```

### IPC

`extension-host.ts` регистрирует top-level (side-effect import):

```ts
ipcMain.handle('kepler:extension:list', () => listExtensions())
ipcMain.handle('kepler:extension:open', (_e, id: string) => openExtension(id))
```

`commands.ts` имеет static команду `dashboard:extension:demo` → lazy-import `openExtension('dashboard')` — это PoC trigger из launcher'а.

### Security

BrowserWindow создаётся с:

```ts
webPreferences: {
  preload: manifest.preload ? path.join(root, id, manifest.preload) : undefined,
  contextIsolation: true,
  nodeIntegration: false,
}
```

- `contextIsolation: true` — `window` extensiion'а изолирован, доступ к Node API только через preload bridge.
- `nodeIntegration: false` — extension не может `require('fs')`.
- Preload — **per-extension**, опциональный. Если есть — extension получает API через `contextBridge.exposeInMainWorld`.

## Phase 4 plan миграции

| Порядок | Апка | Почему | Сложность |
|---|---|---|---|
| 1 | Dashboard | Read-only, минимум IPC, простейшая | низкая |
| 2 | Horologion | Multi-view Vue, IPC для pomodoro/stopwatch state, но без сложных нативных деп | средняя |
| 3 | Delphi | Task UI, persistence через ARK уже есть | средняя |
| 4 | Arrancador | Game library, IPC для resolve exe, sidecar — но через kepler-backend уже централизован | средняя |
| 5 | Eden | TipTap editor + Heart Rust поиск + сложное preload API. **~2 недели alone** | высокая |

Eden — последний, потому что у него самый широкий preload API (titlebar history, store hardening, search, FTS), и Heart требует bundling Rust .node addon.

## Open questions (design decisions для Phase 4)

::: info Эти вопросы — не решены
До начала real миграции апок нужно принять следующие решения:
:::

### ArkClient в extensions

Как extension получает `ArkClient` для работы с ARK?

- **Вариант A — preload bridge.** Kepler-shell владеет одним общим `ArkClient`, expose через preload `contextBridge`. Extension вызывает `window.arkApi.objects.list()`. Плюс — один WS connection. Минус — preload bridge нужно designed для всех ARK операций.
- **Вариант B — extension спавнит свой ArkClient.** Extension получает в preload `keplerLock` (port + bearer) и создаёт свой WS connection через `@kosmos/ark`. Плюс — extensions изолированы. Минус — N WS connections на kepler-backend.

Текущий PoC ничего не expose'ит (Dashboard placeholder — статичный HTML). Решение нужно к моменту миграции Dashboard.

### Permission model

Нужен ли capabilities (manifest declares `permissions: ['ark.read', 'ark.write', 'fs.read']`)?

- Pro: extensions можно будет ставить от third-party.
- Con: пока все extensions = first-party (Kosmos апки), это overengineering.

Решение по умолчанию — **нет permission model в Phase 4**, добавим если появятся third-party extensions.

### Lifecycle стратегия

- **Always-warm**: при старте kepler-shell pre-load'ить N background BrowserWindow для каждого extension'а (hidden). Быстрое открытие, но ~150 MB × N RAM всё время.
- **Lazy (current)**: окно создаётся при первом `openExtension(id)`, остаётся живым пока user не закроет. Reuse через map.
- **Lazy + auto-close**: окно создаётся при open, закрывается через timeout после потери фокуса. Минимум RAM, но cold-start задержка.

Текущая реализация = lazy. Always-warm — оптимизация под Phase 4.5+.

### Update mechanism

Как extensions обновляются?

- Сейчас extensions лежат в `process.resourcesPath/extensions/` — обновляются вместе с kepler.exe (через NSIS).
- Возможный вариант: separate extension marketplace / lazy download в `app.getPath('userData')/extensions/`. Но требует подписи + manifest validation + rollback.

Решение по умолчанию — **bundling с Kepler installer**, ничего отдельно не downloaded.

## Code refs

| Файл | Что |
|---|---|
| `apps/kepler-shell/electron/extension-host.ts` | Loader, IPC handlers, BrowserWindow создание |
| `apps/kepler-shell/electron/commands.ts` | `dashboard:extension:demo` — PoC trigger |
| `apps/kepler-shell/extensions/<id>/manifest.json` | Per-extension манифест |
| `apps/kepler-shell/extensions/<id>/index.html` | Entry HTML |

## Связанные документы

- [Архитектура](/concepts/architecture) — общая картина.
- [Command bus](/concepts/command-bus) — как launcher invoke'ит «ручки» апок (work для standalone-апок и для extensions одинаково).
- [Kepler app](/apps/kepler) — обзор launcher'а целиком.
