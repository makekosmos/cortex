# Extension host

Extension host — production foundation Kepler shell: продуктовые апки рендерятся как **Vue extension bundles** в отдельных BrowserWindow внутри kepler-shell, без собственных Electron .exe.

::: tip Текущий статус — Production foundation (Phase 4 ✅)
Loader (`apps/kepler-shell/electron/extension-host.ts`) и manifest spec используются продакшеном. Мигрированы **4 апки**: Dashboard, Horologion, Delphi, Arrancador (UI subset). Eden намеренно остаётся standalone .exe — миграция запланирована отдельной фазой, см. [Kepler Roadmap → Phase 6](/apps/kepler-roadmap#phase-6).

RAM-эффект миграции зафиксирован в [RAM benchmarks](/concepts/ram-benchmarks): −124 MB Working Set / −209 MB Private Bytes / −4 процесса относительно baseline'а из 4 standalone Electron-апок.
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
  icon?: string             // optional, имя файла иконки (icon.png) рядом с manifest.json
  devPort?: number          // optional, порт Vite dev server'а для HMR (см. dev mode)
  width?: number            // default 900
  height?: number           // default 600
}

export function loadExtensionManifest(id: string): ExtensionManifest | null
export function listExtensions(): ExtensionManifest[]
export function openExtension(id: string): void              // reuse if already open, иначе create
export function extensionIconDataUri(id: string): string | undefined
export function readDevModeSetting(): boolean
export function setExtensionArkBridge(opts: { request, subscribe }): void
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
  "name": "Dashboard",
  "entryHtml": "dist/index.html",
  "icon": "icon.png",
  "devPort": 5180,
  "width": 720,
  "height": 480
}
```

Поля:

- `id`, `name`, `entryHtml` — обязательные.
- `icon` — optional, имя файла иконки внутри директории extension'а (`extensions/<id>/icon.png`). Если есть — `extensionIconDataUri(id)` читает файл и возвращает `data:image/<ext>;base64,...` URI; launcher показывает иконку в open-команде. См. ниже [«App icons»](#app-icons).
- `devPort` — optional, порт Vite dev server'а для HMR. Используется только когда активен developer mode. См. [Extension dev mode](/concepts/extension-dev-mode).
- `width`, `height` — optional, дефолты `900×600`.

### Структура extension директории

```text
apps/kepler-shell/extensions/<id>/
├── manifest.json
├── icon.png             // optional, ссылается через manifest.icon
├── src/                  // Vue sources (dev)
└── dist/                 // build output: index.html + assets/* (entryHtml = "dist/index.html")
```

Shared preload (`extension-preload.mjs`) бандлится отдельно vite-plugin-electron'ом рядом с `main.js` и подгружается во все extension windows.

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
- Preload — **shared** (`dist-electron/extension-preload.mjs`). Bundle'ится vite-plugin-electron'ом и подгружается во все extension windows. Exposes `window.kepler.ark.*` (proxy через main → ArkClient) и `window.kepler.window.{close, minimize, maximize}`.

## App icons

Extension может задать `"icon": "icon.png"` в manifest. Файл лежит рядом с `manifest.json` (`extensions/<id>/icon.png`).

`extensionIconDataUri(id)` (в `electron/extension-host.ts`):

1. Читает manifest, если нет `icon` — возвращает `undefined`.
2. `statSync(iconPath)` — берёт `mtimeMs`.
3. Если в `iconDataUriCache: Map<id, IconCacheEntry>` есть запись с тем же `mtimeMs` — возвращает закешированный data URI.
4. Иначе `readFileSync` → `base64` → собирает `data:image/<ext>;base64,...`, кладёт в кеш с актуальным `mtimeMs`.

```ts
interface IconCacheEntry {
  uri: string | null;
  mtimeMs: number;
}
```

Кеш-инвалидация по mtime важна для dev workflow: пользователь добавил новую `icon.png` в running session → следующий запрос автоматически перечитает файл, рестарт shell'а не нужен. Negative results (битый файл) тоже кешируются как `uri: null` до изменения mtime, чтобы не молотить диск на каждый перерендер launcher'а.

Launcher использует это в `electron/commands.ts` — lazy getter `icon: () => extensionIconDataUri('dashboard')`.

## Crash safety

`extension-host.ts` строит окно через `new BrowserWindow(...)` и регистрирует listener'ы — но `closed` event срабатывает после destroy'я `webContents`, поэтому обращаться к `win.webContents.id` внутри `closed` уже нельзя (`Object has been destroyed`).

Решение — **capture `wcId` до регистрации listener'а**:

```ts
const wcId = win.webContents.id;
webContentsToExtensionId.set(wcId, id);
win.on("closed", () => {
  webContentsToExtensionId.delete(wcId);
  extensionWindows.delete(id);
});
```

Reverse map `webContentsToExtensionId: Map<number, string>` нужен в IPC handler'ах окошек (`kepler:extension:window:*`), чтобы по `e.sender.id` понять, какому extension'у адресован запрос.

## F12 DevTools toggle

В `openExtension` зарегистрирован `before-input-event` listener:

```ts
win.webContents.on("before-input-event", (e, input) => {
  if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
    e.preventDefault();
    try {
      win.webContents.toggleDevTools();
    } catch {
      /* webContents destroyed mid-flight — игнорируем. */
    }
  }
});
```

`try/catch` нужен против race: event может быть в очереди, когда окно уже закрылось.

## Window controls

Shared preload exposes:

```ts
window.kepler.window.close()
window.kepler.window.minimize()
window.kepler.window.maximize()
```

Они шлют `kepler:extension:window:{close,minimize,maximize}` IPC. Main resolves окно через `BrowserWindow.fromWebContents(e.sender)` и вызывает соответствующий метод. Это позволяет extension'ам с `titleBarStyle: "hidden"` рисовать свой titlebar и управлять окном без node integration.

## Developer mode integration

`openExtension(id)` resolver выбирает источник renderer'а:

- Если `isDeveloperModeActive()` (т.е. `KEPLER_DEV=1` **или** `developerMode: true` в `<userData>/kepler-shell-settings.json`) **и** manifest содержит `devPort` — `win.loadURL('http://localhost:<devPort>/')` + auto-open DevTools detached.
- Иначе — `win.loadFile(<root>/<id>/<entryHtml>)` из bundled dist.

Полная схема — [Extension dev mode](/concepts/extension-dev-mode).

## Phase 4 итог миграции

| Апка | Статус | Bundle / замечания |
|---|---|---|
| Dashboard | ✅ | Полная Vue migration, ~83 KB JS. Read-only, ARK через preload bridge. |
| Horologion | ✅ | Полная Vue migration с `horologionApi` shim над `window.kepler.*`. Chunk `pomodoroSettings` ~102 KB. |
| Delphi | ✅ (с долгами) | Vue + memory router, 3483 modules. `window.electronAPI` ссылки в `App.vue` / `ProjectPage` / `SpaceSetup` остались undefined — cleanup в Phase 5. Tailwind plugin не подключён. |
| Arrancador | ⏳ частично | UI subset: LayoutPage + GameCard. Catalogue / Scan / Sqoba / Stats / Settings — **не мигрированы**, native scanner остаётся в legacy. |
| Eden | ❌ outlier | Намеренно standalone .exe. TipTap + Heart Rust + широкий preload API → отдельная фаза (см. [Roadmap → Phase 6](/apps/kepler-roadmap#phase-6)). |

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
- [Extension dev mode](/concepts/extension-dev-mode) — Vite HMR per extension (Raycast-style).
- [RAM benchmarks](/concepts/ram-benchmarks) — измеренный эффект миграции.
- [Kepler app](/apps/kepler) — обзор launcher'а целиком.
