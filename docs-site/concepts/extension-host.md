# Extension host

Extension host — production foundation Kepler shell: продуктовые апки рендерятся как **Vue extension bundles** в отдельных BrowserWindow внутри kepler-shell, без собственных Electron .exe.

::: tip Текущий статус — Production foundation (Phase 4 + 6.0 ✅)
Loader (`platform/desktop/electron/extension-host.ts`) и manifest spec используются продакшеном. Активные Vue-extension апки: Delphi, Arrancador, Eden, Akasha. Dashboard и Focus Session теперь встроенные shell views. Horologion source archived locally and is not active.

RAM-эффект миграции зафиксирован в [RAM benchmarks](/concepts/ram-benchmarks): −124 MB Working Set / −209 MB Private Bytes / −4 процесса относительно baseline'а из 4 standalone Electron-апок (без Eden — Eden replaced standalone в Phase 6.0.A).
:::

## Цель

До Phase 4 каждая Kosmos-апка была отдельная .exe (Eden.exe, Delphi.exe, Horologion.exe, Arrancador.exe, Dashboard.exe). Это значит:

- Память: каждая Electron .exe = ~120-200 MB resident. Пять апок открытых одновременно = ~1 GB только под shell'ы.
- Update: пять отдельных NSIS installer'ов / GitHub Releases.
- Onboarding: пользователь должен установить и запустить каждую апку отдельно.

Phase 4 цель — превратить апки в **extensions**: Vue-бандл + manifest, который загружается kepler-shell'ом в новый BrowserWindow по требованию. Один Electron host на машину (kepler.exe), N extension windows. Update'ы — через kepler.

## Текущая реализация loader'а

`platform/desktop/electron/extension-host.ts`:

```ts
export interface ExtensionManifest {
  id: string;
  name: string;
  kind?: "vue" | "static" | "native" | "raycast";
  entryHtml?: string; // required for vue/static
  native?: {
    executable: string;
    devExecutable?: string;
    cargoPackage?: string;
    args?: string[];
    singleInstance?: boolean;
  };
  icon?: string; // optional, имя файла иконки (icon.png) рядом с manifest.json
  devPort?: number; // optional, порт Vite dev server'а для HMR (см. dev mode)
  width?: number; // default 900
  height?: number; // default 600
  minWidth?: number; // optional, минимальная ширина (например, Eden = 450)
  minHeight?: number; // optional, минимальная высота (например, Eden = 400)
  windowEffect?: "acrylic" | "mica" | "none"; // см. ниже «Window backdrop»
}

export function loadExtensionManifest(id: string): ExtensionManifest | null;
export function listExtensions(): ExtensionManifest[];
export function openExtension(id: string, route?: string): Promise<void>; // reuse if already open, иначе create/spawn
export function extensionIconDataUri(id: string): string | undefined;
export function readDevModeSetting(): boolean;
export function setExtensionArkBridge(opts: { request; subscribe }): void;
```

Resolver. С момента extension installer MVP (2026-05-14) resolution идёт по **priority chain**, а не single root:

```ts
function resolveExtensionRoots(): string[] {
  const roots: string[] = [];
  // 1. Dev source tree — если запущены из repo (highest priority).
  const dev = path.resolve(__dirname, "..", "extensions");
  if (existsSync(dev)) roots.push(dev);
  // 2. User-installed (writable) — основной канал для prod.
  roots.push(path.join(app.getPath("appData"), "Kosmos", "extensions"));
  // 3. Bundled — fallback внутри packaged Kepler.
  if (process.resourcesPath) {
    roots.push(path.join(process.resourcesPath, "extensions"));
  }
  return roots;
}

function resolveExtensionDir(id: string): string | null {
  for (const root of resolveExtensionRoots()) {
    const dir = path.join(root, id);
    if (existsSync(path.join(dir, "manifest.json")) || existsSync(path.join(dir, "package.json")))
      return dir;
  }
  return null;
}
```

Per-id lookup означает, что один extension может быть user-installed (свежий через `ext:install`) или bundled (приехал с installer), и оба источника видны в одном `listExtensions()`. Удаление user-папки (см. [Extension installer](/concepts/extension-installer)) откатывает конкретный extension обратно на bundled.

`listExtensions()` дедуплицирует по `id` — если один и тот же `<id>` присутствует и в user-installed, и в bundled, побеждает первый встреченный (т.е. user-installed override).

Reuse: `Map<id, BrowserWindow>` для `vue/static` и `Map<id, ChildProcess>` для `native`. Если окно уже открыто — `focus()`. Если native child уже жив и `singleInstance !== false` — повторный invoke не spawn'ит второй процесс. На `closed` / `exit` — запись удаляется.

### Raycast-compatible extensions

`kind: "raycast"` — отдельный compatibility foundation для Raycast-style
`package.json` manifests. Такой extension может не иметь `manifest.json`:
loader читает `extensions/<id>/package.json`, валидирует Raycast-поля
`name`, `title`, `version`, `description`, `commands`, `preferences` и
мапит `commands[].mode = "no-view"` в launcher command id
`${extensionId}:${commandName}`.

Kosmos-specific настройки живут в namespace `kosmos`, а не на верхнем уровне:

```json
{
  "name": "copy-tool",
  "title": "Copy Tool",
  "commands": [{ "name": "copy", "title": "Copy", "mode": "no-view" }],
  "kosmos": {
    "permissions": ["userData.read", "userData.write"],
    "commands": {
      "copy": { "entry": "dist/copy.mjs" }
    }
  }
}
```

Phase 0 поддерживает только safe vertical slice:

- private workspace package `@raycast/api` с runtime primitives
  (`List`, `Detail`, `ActionPanel`, `Action`, feedback, clipboard read/write/clear,
  LocalStorage/Cache включая `LocalStorage.allItems()`, preferences, `launchCommand`, `open`, `showInFinder`,
  `trash`) и bridge для `@raycast/api/jsx-runtime` в trusted compiled TSX
  командах;
- `no-view` command runner для trusted sources (`dev` / `bundled`);
- feedback bridge для trusted commands: `showToast` / `showHUD` доходят до
  Raycast host feedback overlay, а `confirmAlert` использует native Electron
  confirmation dialog;
- basic `launchCommand()` lifecycle: trusted Raycast commands can launch
  declared `view`, `no-view`, and regular `open` targets through the shell
  registry, with `LaunchType.LaunchCommand` props forwarded to the target;
- первый `view` host для trusted `List` / `Detail` commands: runner нормализует
  serializable component tree в snapshot, shell renderer показывает список,
  `List.Section`, `List.EmptyView` with footer actions, `List.Item.icon`, `List.Item.accessories`,
  `List.Dropdown` search accessory, поиск,
  controlled `List.searchText` / `List.selectedItemId`, `List.filtering`,
  `List.onSearchTextChange` / `List.onSelectionChange`, `List.isLoading` /
  `Grid.isLoading`, root `Detail`, выбранный item `Detail` markdown,
  root `Detail.actions`, `Detail.Metadata` labels, links, separators,
  tag lists, `ActionPanel`, `ActionPanel.Section`, `ActionPanel.Submenu`,
  `Keyboard.Shortcut.Common` и action `shortcut` labels/key dispatch; `Action.CopyToClipboard` /
  `Action.Paste` / `Action.Pop` / `Action.PopToRoot` /
  `Action.OpenInBrowser` / `Action.Open` /
  `Action.ShowInFinder` / `Action.Trash` / `Action.LaunchCommand`
  выполняются через guarded session IPC, `Action.Push` переключает локальный
  detail target, `List.Item.Detail` aliases share the same metadata renderer,
  navigation actions/generic `Action` с callback выполняются через тот же guarded
  callback registry, а `useNavigation().push/pop/popToRoot` ведёт
  session-level stack в Electron host и шлёт renderer'у guarded snapshot update;
- первый `Form` host для trusted commands: `Form.TextField`,
  `Form.PasswordField`, `Form.TextArea`, `Form.Checkbox`, `Form.Dropdown`,
  `Form.Dropdown.Section`, `Form.Description`, `Form.Separator`,
  `Form.TagPicker`, `Form.DatePicker` with Date defaults/onChange,
  `Form.FilePicker` рендерятся shell renderer'ом;
  field `onChange` callbacks идут через guarded session IPC,
  FilePicker открывает native file dialog через guarded session IPC, а
  `Action.SubmitForm` вызывает trusted callback через guarded session IPC;
  Form footer поддерживает common actions (`CopyToClipboard`, `OpenInBrowser`,
  `Open`, `Paste`, `ShowInFinder`, `Trash`, `LaunchCommand`, generic callback)
  и локальный `Action.Push` в detail target;
- первый `Grid` host для trusted commands: `Grid.Section`, `Grid.Item`,
  `Grid.EmptyView` with footer actions, `Grid.Dropdown` search accessory, card layout,
  image/placeholder preview, search filtering, controlled `Grid.searchText` /
  `Grid.selectedItemId`, `Grid.filtering`, `Grid.onSearchTextChange` /
  `Grid.onSelectionChange`, selection и selected-item `ActionPanel`;
- первый `MenuBarExtra` host для trusted `menu-bar` commands: `package.json`
  `commands[].mode = "menu-bar"` мапится в declared `raycast-menu-bar`,
  runner нормализует serializable `MenuBarExtra` snapshot, shell renderer
  показывает `MenuBarExtra.Section`, `MenuBarExtra.Item`, `MenuBarExtra.Submenu`
  и выполняет item callbacks через guarded session IPC;
- Raycast manifest parser default'ит отсутствующий `commands[].mode` в `view`,
  но отбрасывает явные неизвестные modes, чтобы команда не запускалась в
  неверном host mode;
- user-installed Raycast JS **не исполняется** в main process до появления
  isolated runtime/sandbox. Это намеренный security gate, чтобы не создать
  обход текущей permission model.

Полный React/TSX renderer, `menu-bar`, HMR, OAuth/AI/Grid/Form rich parity,
Store compatibility и untrusted sandbox — отдельные следующие phases.

### Native extensions

`kind: "native"` extension не получает `window.kepler` preload и не грузит
HTML. Shell запускает executable из `manifest.native`:

```json
{
  "id": "my-native-app",
  "name": "My Native App",
  "kind": "native",
  "native": {
    "executable": "bin/my-native-app.exe",
    "devExecutable": "../../target/release/my-native-app.exe",
    "cargoPackage": "my-native-app",
    "singleInstance": true
  }
}
```

Shell добавляет аргументы:

```text
--kosmos-extension-id <id>
--kosmos-user-data-dir <dataDir>/extensions-data/<id>
```

Native apps в Kosmos проектируются как Kosmos-aware, но не Kosmos-required:
тот же executable может запускаться standalone без shell'а. Отличие режимов
задаётся явными аргументами/env от shell'а (`--kosmos-user-data-dir`,
`--kosmos-dev-mode`, `KOSMOS_EXTENSION_DEV_MODE=1`), а не жёсткой зависимостью
от Kepler runtime. Standalone build не должен тащить Kepler backend, ARK RPC,
usage tracking или command bus, если приложение само явно этого не требует.

В `KOSMOS_HEADLESS=1` / `KOSMOS_TEST_MODE=1` native GUI не spawn'ится, чтобы
e2e не показывали окна. Contract tests для native проверяют manifest-команду,
но не ждут Playwright window.

### Deep links через `route`

`openExtension(id, route?)` принимает опциональный второй параметр — Vue Router path (включая query). Доставка route'а **не через URL hash** (extension'ы используют `createMemoryHistory()`, и hash не разбирается роутером), а через IPC:

1. **Cold start** — `openExtension` сохраняет `route` в `extensionWindows.get(id).initialRoute`. После `did-finish-load` main отправляет `kepler:extension:navigation` event в renderer.
2. **Existing window** — `openExtension(id, route)` фокусирует окно и **сразу** отправляет тот же event (без перезагрузки).
3. **Renderer-side** — extension main.ts подписывается через `window.kepler.navigation.onNavigate(route → router.push(route))` и читает initial value через `window.kepler.navigation.initialRoute()` на mount.

Это работает с любым vue-router `history` mode и не зависит от того, передаётся ли в URL hash. Контракт `kepler.navigation` экспортируется через extension preload (см. `platform/desktop/electron/extension-preload.ts`).

Используется в `platform/desktop/electron/commands.ts` для глубоких open-команд:

- `delphi:today` → `openExtension('delphi', '/today')`.
- `eden:note:open-today` → `openExtension('eden', ...)` — Eden открывает сегодняшнюю заметку. (Horologion раньше имел открывающие `horologion:pomodoro` / `horologion:stopwatch`, в 2026-05-19 заменены action-командами `horologion:pomodoro:25` / `:50` / `:stopwatch:start` — `mode:"action"` в manifest, handler стартует таймер.)

Если extension хочет принимать deep links, его `main.ts` должен явно подписаться:

```ts
const nav = window.kepler?.navigation;
if (nav) {
  void nav.initialRoute().then((r) => {
    if (r) void router.push(r);
  });
  nav.onNavigate((r) => void router.push(r));
}
```

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

- `id`, `name` — обязательные.
- `entryHtml` — обязательный для `vue` / `static`.
- `native.executable` — обязательный для `kind: "native"`.
- `icon` — optional, имя файла иконки внутри директории extension'а (`extensions/<id>/icon.png`). Если есть — `extensionIconDataUri(id)` читает файл и возвращает `data:image/<ext>;base64,...` URI; launcher показывает иконку в open-команде. См. ниже [«App icons»](#app-icons).
- `devPort` — optional, порт Vite dev server'а для HMR. Используется только когда активен developer mode. См. [Extension dev mode](/concepts/extension-dev-mode).
- `width`, `height` — optional, дефолты `900×600`.
- `minWidth`, `minHeight` — optional. Передаются в `BrowserWindow` как `minWidth`/`minHeight`. Используются extension'ами, у которых есть собственный layout-breakpoint (например, Eden = `450×400`).
- `windowEffect` — optional. Включает Win32 system backdrop для окна: `"acrylic"` (blur с прозрачностью, Win10/11), `"mica"` (desktop tint, Win11), `"none"` (default). Если задан, `BrowserWindow` создаётся с `backgroundColor: "#00000000"` + `backgroundMaterial: <effect>`. Native `titleBarOverlay` остаётся включённым; renderer учитывает safe-area через `env(titlebar-area-*)`. Renderer обязан использовать прозрачный/полупрозрачный фон body, иначе эффект перекрывается сплошной заливкой. См. [«Window backdrop (acrylic / mica)»](#window-backdrop-acrylic-mica).

### Структура extension директории

```text
extensions/<id>/
├── manifest.json          // Kosmos vue/static/native manifest
├── package.json           // optional Raycast-compatible manifest
├── icon.png             // optional, ссылается через manifest.icon
├── src/                  // Vue sources (dev)
└── dist/                 // build output: index.html + assets/* (entryHtml = "dist/index.html")
```

Shared preload (`extension-preload.mjs`) бандлится отдельно vite-plugin-electron'ом рядом с `main.js` и подгружается во все extension windows.

### IPC

`extension-host.ts` регистрирует top-level (side-effect import):

```ts
ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", (_e, id: string) => openExtension(id));
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

### Runtime permissions

С 2026-06-04 permissions — не только install-dialog metadata. Main-process
IPC проверяет capabilities перед тем как пропустить extension request:

- `kepler:extension:ark:request` → `assertExtensionArkPermission(...)`.
- `kepler:extension:ark:subscribe` → event permission check.
- `kepler:extension:userData:*` → `userData.read` / `userData.write`.
- `kepler:focus-widget:set-state` → `focus.control`.

Trust определяется **по source**, а не по `extension.id`:

- Repo dev tree (`extensions/<id>/`) и bundled resources считаются first-party
  trusted и не обязаны дублировать broad permission list для текущих встроенных
  apps.
- User-installed override в `<dataDir>/extensions/<id>/` считается
  untrusted/default-deny, даже если его id совпадает с `eden`, `delphi` или
  другим first-party id.
- При открытии окна `extension-host.ts` сохраняет snapshot `{ id, source,
manifest.permissions }` по `webContents.id`; последующие IPC checks используют
  этот snapshot, а не пере-resolve'ят папку с диска.

Базовые capabilities:

```text
objects.read
objects.write
objects.write:<type_id>
usage.read
usage.write
commands.register
commands.invoke
focus.control
pomodoro.control
arrancador.scan
arrancador.launch
userData.read
userData.write
hostIndex.read
hostIndex.write
export.read
export.run
dictation.control
sync.read
sync.admin
```

`objects.write:<type_id>` разрешает `upsert_object` для конкретного type и
`delete_object` после lookup типа объекта. Schema/link операции
(`upsert_object_type`, `upsert_object_link`, delete variants) требуют broad
`objects.write`, потому что они меняют структуру object model, а не один object
type. `commands.register` / `commands.unregister` дополнительно требуют, чтобы
все command ids начинались с `${extensionId}:`; extension не может
зарегистрировать или снять чужой namespace.

IPC request запрещает `params.operation`: проверяемая operation должна быть
тем же полем, которое уйдёт в ARK/backend. Иначе malicious extension мог бы
пройти allowlist по первому аргументу и перезаписать operation внутри params.

Auto-update extension'а не должен молча добавлять новые permissions. Если новая
версия manifest'а требует дополнительные capabilities, update flow должен
показывать это как explicit confirmation перед install.

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

Extension windows используют нативные системные кнопки через Electron
`titleBarOverlay`:

```ts
new BrowserWindow({
  frame: true,
  titleBarStyle: "hidden",
  titleBarOverlay: {
    color: "#00000000",
    symbolColor: "#f5f5f5",
    height: 40,
  },
});
```

Renderer не рисует min/max/close сам. `DesktopChrome` / `Titlebar`
учитывают native safe-area через `env(titlebar-area-*)`, поэтому
app-specific кнопки в `#titlebar-trailing` не заезжают под системные
контролы.

Shared preload всё ещё exposes:

```ts
window.kepler.window.close();
window.kepler.window.minimize();
window.kepler.window.maximize();
```

Они шлют `kepler:extension:window:{close,minimize,maximize}` IPC. Main resolves окно через `BrowserWindow.fromWebContents(e.sender)` и вызывает соответствующий метод. API оставлен для programmatic window actions, но не для рендера кастомных кнопок.

Дополнительно к управляющим методам preload exposes текущее состояние maximize:

```ts
window.kepler.window.isMaximized(): Promise<boolean>
window.kepler.window.onMaximizedChange(cb: (value: boolean) => void): () => void
```

- `isMaximized()` → IPC `kepler:extension:window:is-maximized` → `win.isMaximized()` для окна-владельца `webContents`.
- `onMaximizedChange(cb)` подписывается на push-event `kepler:extension:window:maximized-changed`, который main процесс отправляет конкретному окну при `maximize` / `unmaximize`. Возвращает функцию отписки.

### Maximizable toggle

```ts
window.kepler.window.setMaximizable(value: boolean): Promise<void>
```

IPC `kepler:extension:window:set-maximizable` → `win.setMaximizable(value)`.
Используется Eden в zen mode: `setMaximizable(false)` блокирует Win32
native поведение «double-click по titlebar → maximize», чтобы dblclick на
no-drag заголовке отрабатывал кастомный handler (dock-corner toggle), а
не системный maximize. На выходе из zen — `setMaximizable(true)`.

### Dock-corner mode

```ts
window.kepler.window.toggleDockCorner(): Promise<boolean>
window.kepler.window.isDocked(): Promise<boolean>
window.kepler.window.onDockedChange(cb: (value: boolean) => void): () => void
```

IPC: `kepler:extension:window:toggle-dock-corner`,
`kepler:extension:window:is-docked` + push-event
`kepler:extension:window:docked-changed`.

`toggleDockCorner()`:

- Если окно **не** docked — сохраняет текущие bounds, переводит окно в
  floating widget: 360×560, прижато к правому верхнему углу активного
  display'я (`marginX=12`, `marginTop=12`), `setAlwaysOnTop(true)`,
  `setSkipTaskbar(true)`.
- Если docked — восстанавливает saved bounds, `setAlwaysOnTop(false)`,
  `setSkipTaskbar(false)`.
- В обоих случаях резолвится в новый docked-state + main отправляет
  `docked-changed` event в renderer.

Используется только Eden (в zen mode по dblclick на title). Если другой
extension захочет — paттерн полностью переиспользуем, никакой Eden-specific
логики в shell нет.

::: warning Drag-region и dblclick
Electron `-webkit-app-region: drag` перехватывает pointer events для
Win32 window drag — DOM `click` / `dblclick` на таком элементе **не
срабатывают**. Чтобы dblclick handler сработал, нужен child-элемент с
явным `-webkit-app-region: no-drag` (у Eden — `.eden-titlebar-title`
span).
:::

## Window backdrop (acrylic / mica)

Когда manifest задаёт `"windowEffect": "acrylic"` (или `"mica"`), `BrowserWindow` создаётся с:

```ts
backgroundColor: "#00000000",     // полностью прозрачный
backgroundMaterial: "acrylic",    // или "mica"
```

Это включает Win32 системный backdrop под окном. Чтобы он был виден:

1. Renderer body должен иметь прозрачный (или полупрозрачный) фон. В Eden — `.app-container` сплошной только вне zen mode; в zen mode `.app-container.focus-mode-active` получает `backdrop-filter: blur(24px) saturate(140%)` + полупрозрачный фон, а все вложенные surface'ы (titlebar/sidebar/content/editor/ProseMirror) форсятся в `background: transparent !important`.
2. **Caveat — backdrop-filter и containing block.** `backdrop-filter` создаёт новый containing block для `position: fixed` потомков. Если применить его на верхнем `body` / `#root`, fixed-позиционируемые элементы (overlay'и, поиск, toast'ы) начнут позиционироваться относительно этого узла, а не viewport'а — ломается, например, Eden `SearchOverlay`. Решение — навешивать `backdrop-filter` на промежуточный layer (`.app-container.focus-mode-active`), который сам не содержит fixed overlay'ев.

Используется сейчас только в Eden zen mode. См. [Eden → Zen mode + acrylic](/apps/eden#zen-mode-acrylic).

## User data

Каждый extension получает свой персональный writable путь в `<APPDATA>/Kosmos/extensions-data/<id>/`, который **не затрагивается** при install/uninstall extension'а (если пользователь явно не передал `--purge-data`). Это разделение «код vs user data» — см. [Extension installer → Persistent user data](/concepts/extension-installer#persistent-user-data).

### Preload API

```ts
window.kepler.userData = {
  readJson<T>(name: string): Promise<T | null>,
  writeJson<T>(name: string, value: T): Promise<void>,
  readFile(name: string): Promise<string | null>,
  writeFile(name: string, content: string): Promise<void>,
  path(): Promise<string>,  // absolute path к <APPDATA>/Kosmos/extensions-data/<id>/
};
```

`name` — имя файла относительно user data dir. Запрещены символы кроме `[A-Za-z0-9._-]` и leading dot — path traversal заблокирован на стороне main процесса в модуле `extension-user-data` под `platform/desktop/electron/`.

```ts
// Пример из extension renderer
const settings = (await window.kepler.userData.readJson<{ accentColor: string }>(
  "settings.json",
)) ?? { accentColor: "#7c3aed" };
settings.accentColor = "#22c55e";
await window.kepler.userData.writeJson("settings.json", settings);
```

### Window state

`platform/desktop/electron/extension-host.ts → openExtension(id)` автоматически:

1. Читает `<extensions-data>/<id>/window-state.json` перед созданием `BrowserWindow`.
2. Если saved bounds валидные (попадают хотя бы частично в активный display) — применяет.
3. Если `isMaximized: true` — максимизирует после `ready-to-show`.
4. Подписывается на `resized` / `moved` (debounced 500ms) + `maximize` / `unmaximize` / `close` — пишет актуальные bounds в файл.

Extension ничего не делает — это shell-level автоматика.

```json
{
  "width": 1200,
  "height": 800,
  "x": 100,
  "y": 50,
  "isMaximized": false,
  "savedAt": "2026-05-14T12:34:56Z"
}
```

## Developer mode integration

`openExtension(id)` resolver выбирает источник renderer'а:

- Если `isDeveloperModeActive()` (только `developerMode: true` в `<userData>/kepler-shell-settings.json` — env var `KEPLER_DEV` **не** активирует extension dev mode, см. ниже) **и** manifest содержит `devPort` — `win.loadURL('http://localhost:<devPort>/#<route>')` + auto-open DevTools detached.
- Иначе — `win.loadFile(<root>/<id>/<entryHtml>, { hash: route })` из bundled dist.

::: warning KEPLER_DEV больше не включает extension HMR
Раньше `isDeveloperModeActive()` возвращал `true` при `process.env.KEPLER_DEV === "1"`. Это ломало `bun run --cwd platform/desktop dev` (который сам выставляет `KEPLER_DEV=1`): shell пытался грузить extensions с Vite dev server'а, который не запущен → пустые extension окна. Сейчас `isDeveloperModeActive()` смотрит **только** настройку `developerMode` из `kepler-shell-settings.json`. Env var `KEPLER_DEV=1` влияет только на shell-level dev (DevTools шелла, dev URL шелла), но не на extension loader. Settings UI **отображает** `KEPLER_DEV=1 || developerMode` (чтобы toggle в dev-сессии выглядел консистентно), но это исключительно индикатор статуса — extension loader игнорирует env.
:::

Полная схема — [Extension dev mode](/concepts/extension-dev-mode).

## Итог миграции (Phase 4 + 6.0)

| Апка       | Статус          | Bundle / замечания                                                                                                                                                                     |
| ---------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Dashboard  | ✅              | Полная Vue migration, ~83 KB JS. Read-only, ARK через preload bridge.                                                                                                                  |
| Horologion | ✅              | Полная Vue migration с `horologionApi` shim над `window.kepler.*`. Chunk `pomodoroSettings` ~102 KB.                                                                                   |
| Delphi     | ✅ (с долгами)  | Vue + memory router, 3483 modules. `electronAPI` shim над `window.kepler.*`. Tailwind plugin подключён.                                                                                |
| Arrancador | ✅ (2026-05-18) | Все 4 страницы оживлены + backend в `platform/runtime/src/arrancador/`: scanner Steam+Epic, launcher (Steam URL + exe spawn), RAWG client, SQOBA save backups.                         |
| Eden       | ✅ (Phase 6.0)  | TipTap editor + Pinia + `kepler-api-shim` над `window.kepler.ark`. Main bundle ~353KB, lazy Editor chunk ~1.36MB. Hevy / code-tools / vault picker / Heart Rust удалены в Phase 6.0.A. |

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

Capability model включён в runtime (см. [Runtime permissions](#runtime-permissions)).
Open question остался не в enforcement, а в UX: marketplace/auto-update должны
сравнивать старый и новый `manifest.permissions` и требовать явного согласия,
если update добавляет capabilities.

### Lifecycle стратегия

- **Always-warm**: при старте kepler-shell pre-load'ить N background BrowserWindow для каждого extension'а (hidden). Быстрое открытие, но ~150 MB × N RAM всё время.
- **Lazy (current)**: окно создаётся при первом `openExtension(id)`, остаётся живым пока user не закроет. Reuse через map.
- **Lazy + auto-close**: окно создаётся при open, закрывается через timeout после потери фокуса. Минимум RAM, но cold-start задержка.

Текущая реализация = lazy. Always-warm — оптимизация под Phase 4.5+.

### Update mechanism

Как extensions обновляются?

- Built-ins едут с Kepler NSIS installer'ом в `process.resourcesPath/extensions/` — обновляются вместе с kepler.exe.
- **User-installed override (с 2026-05-14)**: ставится через CLI в `%APPDATA%\Kosmos\extensions\<id>\` и перекрывает bundled версию для этого `id`. См. [Extension installer](/concepts/extension-installer). Это закрывает потребность «обновить конкретное приложение, не пересобирая весь shell».
- Auto-update / version compare / `.kext` пакетный формат — open для будущей итерации, см. [Kepler Roadmap](/apps/kepler-roadmap).

## Code refs

| Файл                                          | Что                                          |
| --------------------------------------------- | -------------------------------------------- |
| `platform/desktop/electron/extension-host.ts` | Loader, IPC handlers, BrowserWindow создание |
| `platform/desktop/electron/commands.ts`       | `dashboard:extension:demo` — PoC trigger     |
| `extensions/<id>/manifest.json`               | Per-extension Kosmos манифест                |
| `extensions/<id>/package.json`                | Raycast-compatible package manifest          |
| `extensions/<id>/index.html`                  | Entry HTML                                   |

## Связанные документы

- [Архитектура](/concepts/architecture) — общая картина.
- [Command bus](/concepts/command-bus) — как launcher invoke'ит «ручки» апок (work для standalone-апок и для extensions одинаково).
- [Extension dev mode](/concepts/extension-dev-mode) — Vite HMR per extension (Raycast-style).
- [RAM benchmarks](/concepts/ram-benchmarks) — измеренный эффект миграции.
- [Kepler app](/apps/kepler) — обзор launcher'а целиком.
