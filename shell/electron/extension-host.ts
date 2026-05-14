// Extension host — управляет lifecycle Vue/static extension'ов внутри Kepler.
//
// Phase 4 contract для extension'ов:
//
//   extensions/<id>/
//     manifest.json         { id, name, kind: "vue" | "static",
//                             entryHtml: "dist/index.html" | "index.html",
//                             preload?, width, height, minWidth?, minHeight? }
//     src/                  (только для kind: "vue")
//       main.ts             // createApp(App).mount("#app")
//       App.vue
//       ...
//     index.html            (для "vue" — точка входа vite, ссылается на /src/main.ts;
//                            для "static" — финальный готовый html)
//     dist/                 (для "vue" — build output, entryHtml: "dist/index.html")
//
// Vue extension'ы получают `window.kepler` namespace через shared preload
// (extension-preload.mjs). API exposes:
//   - kepler.ark.request(operation, params)   — RPC к ARK через main proxy
//   - kepler.ark.subscribe(event, handler)    — события (commands_changed и т.п.)
//   - kepler.window.{close,minimize,maximize} — управление окном
//   - kepler.meta.id()                        — id текущего extension'а
//
// kind: "static" (legacy PoC) — preload не используется по умолчанию;
// extension сам отвечает за всю свою логику.

import {
  app,
  BrowserWindow,
  ipcMain,
  screen,
  type WebContents,
} from "electron";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ESM shim — __dirname / __filename не определены в Node ESM bundles.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export type ExtensionKind = "vue" | "static";

export interface ExtensionManifest {
  id: string;
  name: string;
  kind?: ExtensionKind;
  entryHtml: string;
  /**
   * Путь к preload-скрипту:
   *  - для "vue" по умолчанию используется shared preload из dist-electron;
   *  - для "static" значение интерпретируется относительно директории
   *    extension'а (legacy PoC mode).
   *  - специальное значение "kepler-extension-preload.mjs" принудительно
   *    использует shared preload.
   */
  preload?: string;
  /**
   * Относительный путь к иконке extension'а внутри его директории
   * (обычно `icon.png`). Используется launcher'ом для open-команд.
   */
  icon?: string;
  /**
   * Порт Vite dev server'а в developer mode. Если задан и активирован
   * developer mode (env `KEPLER_DEV=1` или toggle в Settings) — extension
   * грузится с `http://localhost:<devPort>/` вместо `dist/index.html`.
   */
  devPort?: number;
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
}

interface ExtensionWindowEntry {
  win: BrowserWindow;
  id: string;
}

const extensionWindows = new Map<string, ExtensionWindowEntry>();
// Reverse map: webContents.id → extension id. Нужен, чтобы из IPC handler'а
// определить, какое окно отправило запрос (kepler.window.close и т.п.).
const webContentsToExtensionId = new Map<number, string>();

// ArkClient injected lazily из main.ts через `setArkClient`. Если null —
// extension'ы получают ошибку при попытке ARK-запроса.
type ArkRequestFn = (req: Record<string, unknown>) => Promise<unknown>;
type ArkSubscribeFn = (
  event: string,
  handler: (payload: unknown) => void,
) => () => void;

let arkRequest: ArkRequestFn | null = null;
let arkSubscribe: ArkSubscribeFn | null = null;

/**
 * Регистрируется из main.ts после init ArkClient'а. extension-host остаётся
 * loosely-coupled — не импортирует ArkClient напрямую и не дублирует логику
 * выбора self-managed / kepler-managed режима.
 */
export function setExtensionArkBridge(opts: {
  request: ArkRequestFn | null;
  subscribe: ArkSubscribeFn | null;
}): void {
  arkRequest = opts.request;
  arkSubscribe = opts.subscribe;
}

// ---------------------------------------------------------------------------
// Developer mode
// ---------------------------------------------------------------------------

/**
 * Читает `developerMode: boolean` из `<userData>/kepler-shell-settings.json`.
 * Если файла нет / повреждён — возвращает false. Sync read: вызывается
 * редко (на open extension), значит выигрыша от async нет.
 */
export function readDevModeSetting(): boolean {
  try {
    const file = path.join(app.getPath("userData"), "kepler-shell-settings.json");
    if (!existsSync(file)) return false;
    const json = JSON.parse(readFileSync(file, "utf8")) as {
      developerMode?: boolean;
    };
    return !!json.developerMode;
  } catch {
    return false;
  }
}

function isDeveloperModeActive(): boolean {
  return process.env.KEPLER_DEV === "1" || readDevModeSetting();
}

// Priority chain для resolution extension-папок. Higher priority first.
//
//   1. Dev source tree (`<repoRoot>/extensions/`) — если папка
//      существует. Это означает, что мы запущены из repo (developer flow).
//   2. User-installed (`%APPDATA%\Kosmos\extensions\<id>\`) — основной канал
//      для prod: пользователь устанавливает / обновляет extension через CLI
//      или (в будущем) через UI, копия живёт в writable location.
//   3. Bundled (`<resourcesPath>/extensions/<id>\`) — fallback для packaged
//      сборок: built-in extensions едут с Kepler installer'ом, user-installed
//      их перекрывает, удаление user-папки откатывает на bundled.
//
// Per-id lookup (`resolveExtensionDir`) обходит цепочку и возвращает первый
// корень, где есть `manifest.json`. Это позволяет смешивать: Dashboard может
// быть user-installed, а Horologion — bundled.
function resolveExtensionRoots(): string[] {
  const roots: string[] = [];
  // Repo dev tree: __dirname is shell/electron/ (or shell/dist-electron/),
  // extensions are at <repoRoot>/extensions/ — i.e. ../../extensions/ from here.
  const dev = path.resolve(__dirname, "..", "..", "extensions");
  if (existsSync(dev)) roots.push(dev);
  const userRoot = path.join(
    app.getPath("appData"),
    "Kosmos",
    "extensions",
  );
  if (!roots.includes(userRoot)) roots.push(userRoot);
  if (process.resourcesPath) {
    const bundled = path.join(process.resourcesPath, "extensions");
    if (!roots.includes(bundled)) roots.push(bundled);
  }
  return roots;
}

/**
 * Path к директории, куда CLI installer пишет user-installed extensions.
 * Экспортируется, чтобы install/uninstall scripts могли импортировать его
 * (а не дублировать path-логику).
 */
export function userExtensionsRoot(): string {
  return path.join(app.getPath("appData"), "Kosmos", "extensions");
}

function resolveExtensionDir(id: string): string | null {
  for (const root of resolveExtensionRoots()) {
    const dir = path.join(root, id);
    if (existsSync(path.join(dir, "manifest.json"))) return dir;
  }
  return null;
}

function resolveSharedPreloadPath(): string {
  // Bundled by vite-plugin-electron alongside main.js: dist-electron/extension-preload.mjs
  return path.join(__dirname, "extension-preload.mjs");
}

export function loadExtensionManifest(id: string): ExtensionManifest | null {
  const dir = resolveExtensionDir(id);
  if (!dir) return null;
  const manifestPath = path.join(dir, "manifest.json");
  try {
    return JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
  } catch (e) {
    console.error(`[kepler-shell] extension manifest invalid: ${id}`, e);
    return null;
  }
}

// In-memory cache иконок с mtime-инвалидацией — если файл иконки изменился
// на диске (user добавил новую), кеш автоматически перечитает на следующий
// запрос. Это важно для dev workflow когда иконки меняются в running session.
interface IconCacheEntry {
  uri: string | null;
  mtimeMs: number;
}
const iconDataUriCache = new Map<string, IconCacheEntry>();

/**
 * Возвращает icon extension'а как `data:image/png;base64,...` URI, или undefined
 * если у extension'а нет icon (нет поля в manifest или файл отсутствует).
 * Кеширует по mtime файла — повторные вызовы дешёвые, обновление файла
 * автоматически перечитывается.
 */
export function extensionIconDataUri(id: string): string | undefined {
  const manifest = loadExtensionManifest(id);
  if (!manifest || !manifest.icon) return undefined;
  const dir = resolveExtensionDir(id);
  if (!dir) return undefined;
  const iconPath = path.join(dir, manifest.icon);
  if (!existsSync(iconPath)) return undefined;
  const stat = statSync(iconPath);
  const cached = iconDataUriCache.get(id);
  if (cached && cached.mtimeMs === stat.mtimeMs) {
    return cached.uri ?? undefined;
  }
  try {
    const buf = readFileSync(iconPath);
    const ext = path.extname(iconPath).toLowerCase();
    const mime =
      ext === ".svg" ? "image/svg+xml" : ext === ".jpg" || ext === ".jpeg" ? "image/jpeg" : "image/png";
    const uri = `data:${mime};base64,${buf.toString("base64")}`;
    iconDataUriCache.set(id, { uri, mtimeMs: stat.mtimeMs });
    return uri;
  } catch (e) {
    console.error(`[kepler-shell] failed to read icon for ${id}:`, e);
    iconDataUriCache.set(id, { uri: null, mtimeMs: stat.mtimeMs });
    return undefined;
  }
}

export function listExtensions(): ExtensionManifest[] {
  // Collect ids из всех roots; dedup по id, выигрывает первый встреченный
  // (priority order — см. resolveExtensionRoots).
  const seen = new Set<string>();
  const out: ExtensionManifest[] = [];
  for (const root of resolveExtensionRoots()) {
    if (!existsSync(root)) continue;
    try {
      const entries = readdirSync(root, { withFileTypes: true });
      for (const entry of entries) {
        if (!entry.isDirectory()) continue;
        if (seen.has(entry.name)) continue;
        const m = loadExtensionManifest(entry.name);
        if (m) {
          seen.add(entry.name);
          out.push(m);
        }
      }
    } catch {
      /* ignore unreadable root */
    }
  }
  return out;
}

function resolvePreloadForManifest(
  manifest: ExtensionManifest,
  extensionDir: string,
): string | undefined {
  const kind: ExtensionKind = manifest.kind ?? "static";
  // Shared preload по умолчанию для Vue extension'ов.
  if (!manifest.preload) {
    return kind === "vue" ? resolveSharedPreloadPath() : undefined;
  }
  // Спец-значение → shared preload.
  if (manifest.preload === "kepler-extension-preload.mjs") {
    return resolveSharedPreloadPath();
  }
  // Иначе — относительный путь внутри extension dir (legacy / custom).
  return path.join(extensionDir, manifest.preload);
}

function resolveEntryHtml(
  manifest: ExtensionManifest,
  extensionDir: string,
): string {
  // entryHtml интерпретируется относительно extension dir. Для vue это обычно
  // "dist/index.html" (после vite build), для static — "index.html".
  return path.join(extensionDir, manifest.entryHtml);
}

export function openExtension(id: string): void {
  const existing = extensionWindows.get(id);
  if (existing && !existing.win.isDestroyed()) {
    existing.win.focus();
    return;
  }
  const manifest = loadExtensionManifest(id);
  if (!manifest) {
    console.warn(`[kepler-shell] extension not found: ${id}`);
    return;
  }
  const extensionDir = resolveExtensionDir(id);
  if (!extensionDir) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const useDev = isDeveloperModeActive() && !!manifest.devPort;
  const entryHtml = resolveEntryHtml(manifest, extensionDir);
  if (!useDev && !existsSync(entryHtml)) {
    console.error(
      `[kepler-shell] extension '${id}' entryHtml not found: ${entryHtml}` +
        ` — для Vue extension'а сначала запусти build (bun run build:extensions).`,
    );
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;
  const width = manifest.width ?? 1200;
  const height = manifest.height ?? 800;
  const preload = resolvePreloadForManifest(manifest, extensionDir);

  const win = new BrowserWindow({
    width,
    height,
    minWidth: manifest.minWidth ?? 800,
    minHeight: manifest.minHeight ?? 600,
    x: Math.round((display.width - width) / 2),
    y: Math.round((display.height - height) / 2),
    show: true,
    title: manifest.name,
    backgroundColor: "#1a1a1a",
    // Стандартное окно с custom titlebar (overlay для управления окном).
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#1a1a1a",
      symbolColor: "#cccccc",
      height: 36,
    },
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  // Capture webContents.id ДО регистрации listener'ов. После 'closed' event
  // BrowserWindow.webContents уже destroyed и обращение к нему кидает
  // "Object has been destroyed".
  const wcId = win.webContents.id;
  webContentsToExtensionId.set(wcId, id);
  win.on("closed", () => {
    webContentsToExtensionId.delete(wcId);
    extensionWindows.delete(id);
  });
  extensionWindows.set(id, { win, id });

  // F12 toggles DevTools для extension window (без модификаторов).
  // try/catch — на случай race condition при закрытии окна, когда event ещё
  // в очереди, а webContents уже destroyed.
  win.webContents.on("before-input-event", (e, input) => {
    if (
      input.key === "F12" &&
      !input.alt &&
      !input.control &&
      !input.shift &&
      !input.meta
    ) {
      e.preventDefault();
      try {
        win.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight — игнорируем. */
      }
    }
  });

  if (useDev && manifest.devPort) {
    const devUrl = `http://localhost:${manifest.devPort}/`;
    console.log(`[kepler-shell] extension '${id}' dev mode → ${devUrl}`);
    void win.loadURL(devUrl);
    win.webContents.openDevTools({ mode: "detach" });
  } else {
    void win.loadFile(entryHtml);
  }
}

function windowForSender(sender: WebContents): BrowserWindow | null {
  const win = BrowserWindow.fromWebContents(sender);
  return win && !win.isDestroyed() ? win : null;
}

function extensionIdForSender(sender: WebContents): string | null {
  return webContentsToExtensionId.get(sender.id) ?? null;
}

// ---------------------------------------------------------------------------
// IPC: list / open (kept from previous PoC contract)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", (_e, id: string) => openExtension(id));

// ---------------------------------------------------------------------------
// IPC: ARK proxy — extension renderer → main → ArkClient
// ---------------------------------------------------------------------------

ipcMain.handle(
  "kepler:extension:ark:request",
  async (_e, operation: string, params?: Record<string, unknown>) => {
    if (!arkRequest) {
      throw new Error("ark bridge not ready");
    }
    const req: Record<string, unknown> = { operation, ...(params ?? {}) };
    return arkRequest(req);
  },
);

// Extension subscribes; main forwards events to that extension's webContents.
// Channel name encodes event name so multiple subscriptions on the same
// webContents do not collide.
const extensionEventUnsubscribers = new Map<string, () => void>();

ipcMain.handle(
  "kepler:extension:ark:subscribe",
  (e, event: string) => {
    if (!arkSubscribe) {
      throw new Error("ark bridge not ready");
    }
    const sender = e.sender;
    const key = `${sender.id}:${event}`;
    if (extensionEventUnsubscribers.has(key)) {
      // Idempotent — повторная подписка no-op.
      return true;
    }
    const unsubscribe = arkSubscribe(event, (payload) => {
      if (!sender.isDestroyed()) {
        sender.send(`kepler:extension:ark:event:${event}`, payload);
      }
    });
    extensionEventUnsubscribers.set(key, unsubscribe);
    // Cleanup при закрытии renderer'а.
    sender.once("destroyed", () => {
      const u = extensionEventUnsubscribers.get(key);
      if (u) {
        u();
        extensionEventUnsubscribers.delete(key);
      }
    });
    return true;
  },
);

ipcMain.handle(
  "kepler:extension:ark:unsubscribe",
  (e, event: string) => {
    const key = `${e.sender.id}:${event}`;
    const unsubscribe = extensionEventUnsubscribers.get(key);
    if (unsubscribe) {
      unsubscribe();
      extensionEventUnsubscribers.delete(key);
    }
    return true;
  },
);

// ---------------------------------------------------------------------------
// IPC: meta / window controls (extension renderer → main)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:meta:id", (e) => extensionIdForSender(e.sender));

ipcMain.handle("kepler:extension:window:close", (e) => {
  const win = windowForSender(e.sender);
  if (win) win.close();
});

ipcMain.handle("kepler:extension:window:minimize", (e) => {
  const win = windowForSender(e.sender);
  if (win) win.minimize();
});

ipcMain.handle("kepler:extension:window:maximize", (e) => {
  const win = windowForSender(e.sender);
  if (!win) return;
  if (win.isMaximized()) {
    win.unmaximize();
  } else {
    win.maximize();
  }
});

// ---------------------------------------------------------------------------
// IPC: host-action (extension → kepler-shell host action)
// ---------------------------------------------------------------------------

// Reserved для будущих host-action типа "show settings", "focus launcher" и т.п.
// Сейчас просто логирует и возвращает false (action not handled).
ipcMain.handle(
  "kepler:extension:invoke-host",
  (_e, action: string, _payload?: unknown) => {
    console.error(`[kepler-shell] extension invoke-host: ${action} (no handler)`);
    return false;
  },
);
