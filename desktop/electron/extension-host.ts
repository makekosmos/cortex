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

import { app, BrowserWindow, dialog, ipcMain, type WebContents } from "electron";
import {
  ensureUserDataDir,
  isHeadlessOrTest,
  listExtensions,
  loadExtensionManifest,
  resolveExtensionLocation,
  type ExtensionManifest,
} from "./extension-manifest";
export { findDeclaredCommand, loadDeclaredCommands } from "./extension-declared-commands";
export { extensionUserDataDir } from "./extension-manifest";
export type { ExtensionManifest } from "./extension-manifest";
export type { DeclaredCommand } from "./extension-declared-commands";
import type { ExtensionWindowProfile } from "./extension-window-profile";
import {
  assertExtensionHostPermission,
  type ExtensionSource as ExtensionPermissionSource,
} from "./extension-permissions";
import { registerExtensionMarkdownIpc } from "./extension-markdown-ipc";
import { registerExtensionUserDataIpc } from "./extension-user-data-ipc";
import { registerExtensionInstallerIpc } from "./extension-installer-ipc";
import { registerMarketplaceIpc, startPeriodicCatalogCheck } from "./extension-marketplace";
import { clearExtensionWindowIpcState, registerExtensionWindowIpc } from "./extension-window-ipc";
import { registerExtensionArkIpc } from "./extension-ark-ipc";
import { registerExtensionImageColorIpc } from "./extension-image-color-ipc";
import { registerExtensionBookMetadataIpc } from "./extension-book-metadata-ipc";
import { isNativeExtensionRunning, openNativeExtension } from "./extension-native-runner";
import { openExtensionBrowserWindow } from "./extension-browser-window";
import type { JsonValue } from "./extension-permissions";

export { setExtensionArkBridge, setExtensionArkBridgeReadyTimeoutMs } from "./extension-ark-ipc";

interface ExtensionWindowEntry {
  win: BrowserWindow;
  id: string;
  windowKey: string;
  /** Если true — close скрывает окно вместо destroy (см. ExtensionManifest.keepAliveInBackground). */
  keepAliveInBackground: boolean;
  /** Initial route переданный в `openExtension(id, route)` для cold start.
      Renderer читает через `kepler.navigation.initialRoute()` на mount. */
  initialRoute?: string;
}

const extensionWindows = new Map<string, ExtensionWindowEntry>();

// При quit'е приложения keepAliveInBackground extension'ы должны реально
// уничтожиться, а не зацикливать preventDefault → app.exit вис. before-quit
// взводит этот флаг до того как BrowserWindow'ы получат close.
let isAppQuitting = false;
app.on("before-quit", () => {
  isAppQuitting = true;
});
interface ExtensionRendererContext {
  id: string;
  version?: string;
  windowKey: string;
  source: ExtensionPermissionSource;
  manifestPermissions?: readonly string[];
}

// Reverse map: webContents.id → extension context. Нужен, чтобы из IPC handler'а
// определить, какое окно отправило запрос, и какие permissions были у кода при
// открытии. Source snapshot важен: user-installed override нельзя доверять по id.
const webContentsToExtensionContext = new Map<number, ExtensionRendererContext>();

/** Is extension currently running (window exists, not destroyed)? */
export function isExtensionRunning(id: string): boolean {
  if (isNativeExtensionRunning(id)) return true;
  return Array.from(extensionWindows.values()).some(
    (entry) => entry.id === id && !entry.win.isDestroyed(),
  );
}

/**
 * Поднять уже существующее окно extension'а на передний план.
 *
 * Why not just `win.focus()`:
 *   - Если minimized — focus() на Windows не разворачивает окно (Electron
 *     возвращает focus IF taskbar отвечает; без restore() окно остаётся в трее).
 *   - Если hidden (`win.isVisible() === false`, например после `win.hide()`) —
 *     focus() no-op'ает; нужен `show()`.
 *   - Если Kepler не foreground-app (юзер invoke'нул через global hotkey из
 *     другого приложения) — Win32 запрещает forceForegroundWindow от
 *     non-foreground процесса. Стандартный workaround — toggle
 *     `setAlwaysOnTop(true) → setAlwaysOnTop(false)` за один tick: окно
 *     поднимается, затем флаг снимается, обычный z-order behavior сохраняется.
 *
 * Order matters: restore() → show() → AOT-toggle → focus(). Restore первым,
 * иначе show()/focus() могут опять прилипнуть к taskbar.
 */
function focusExistingExtensionWindow(win: BrowserWindow): void {
  if (win.isDestroyed()) return;
  // Headless / test mode: окна не показываем, Playwright работает через
  // webContents без paint'а.
  if (isHeadlessOrTest()) {
    return;
  }
  try {
    if (win.isMinimized()) win.restore();
    if (!win.isVisible()) win.show();
    // Win32 quirk — toggle AOT чтобы поднять окно на передний план даже когда
    // Kepler не foreground-app. На macOS/Linux обычно достаточно focus(), но
    // toggle безопасен (короткая вспышка AOT не визуально заметна).
    const wasAlwaysOnTop = win.isAlwaysOnTop();
    if (!wasAlwaysOnTop) {
      win.setAlwaysOnTop(true);
      win.setAlwaysOnTop(false);
    }
    win.focus();
    win.moveTop();
  } catch (e) {
    // Race: окно могло destroy'нуться между isDestroyed-check и операцией.
    console.warn("[kepler-shell] focusExistingExtensionWindow failed:", e);
  }
}

// Dedupe concurrent openExtension calls для одного и того же id. Без этого
// два быстрых invoke могли создать два BrowserWindow'а: между existing-check
// и `extensionWindows.set` теперь есть `await resolveExtensionSource` (probe).
const openInflight = new Map<string, Promise<void>>();

export function openExtension(
  id: string,
  route?: string,
  windowKey = id,
  profile: ExtensionWindowProfile = "default",
): Promise<void> {
  const existing = openInflight.get(windowKey);
  if (existing) return existing;
  const promise = openExtensionImpl(id, route, windowKey, profile).finally(() => {
    openInflight.delete(windowKey);
  });
  openInflight.set(windowKey, promise);
  return promise;
}

export function reloadExtensionWindow(id: string): boolean {
  const entry = extensionWindows.get(id);
  if (!entry || entry.win.isDestroyed()) return false;
  entry.win.webContents.reloadIgnoringCache();
  return true;
}

async function openExtensionImpl(
  id: string,
  route?: string,
  windowKey = id,
  profile: ExtensionWindowProfile = "default",
): Promise<void> {
  const manifest = loadExtensionManifest(id);
  if (!manifest) {
    console.warn(`[kepler-shell] extension not found: ${id}`);
    return;
  }
  if (manifest.kind === "native") {
    await openNativeExtension(id, manifest, route);
    return;
  }
  if (manifest.kind === "command-extension") {
    console.warn(
      `[kepler-shell] Command view commands are not implemented yet: ${id}${route ? ` (${route})` : ""}`,
    );
    return;
  }
  await openExtensionBrowserWindow({
    id,
    route,
    windowKey,
    profile,
    manifest,
    isAppQuitting: () => isAppQuitting,
    extensionWindows,
    webContentsToExtensionContext,
    clearWindowIpcState: clearExtensionWindowIpcState,
    focusExistingWindow: focusExistingExtensionWindow,
  });
}
export function commandRuntimeContext(id: string): {
  manifest: ExtensionManifest;
  dir: string;
  source: ExtensionPermissionSource;
} | null {
  const manifest = loadExtensionManifest(id);
  const location = resolveExtensionLocation(id);
  if (!manifest || !location || manifest.kind !== "command-extension") return null;
  return { manifest, dir: location.dir, source: location.source };
}

function windowForSender(sender: WebContents): BrowserWindow | null {
  const win = BrowserWindow.fromWebContents(sender);
  return win && !win.isDestroyed() ? win : null;
}

function extensionIdForSender(sender: WebContents): string | null {
  return webContentsToExtensionContext.get(sender.id)?.id ?? null;
}

function extensionWindowKeyForSender(sender: WebContents): string | null {
  return webContentsToExtensionContext.get(sender.id)?.windowKey ?? null;
}

function initialRouteForSender(sender: WebContents): string | null {
  const windowKey = extensionWindowKeyForSender(sender);
  if (!windowKey) return null;
  return extensionWindows.get(windowKey)?.initialRoute ?? null;
}

function extensionContextForSender(sender: WebContents): ExtensionRendererContext {
  const context = webContentsToExtensionContext.get(sender.id);
  if (!context) {
    throw new Error("[kepler-shell] sender is not an extension");
  }
  return context;
}

function assertExtensionSenderHostPermission(
  sender: WebContents,
  capability:
    | "userData.read"
    | "userData.write"
    | "focus.control"
    | "network.read"
    | "dialogs.directory"
    | "markdownFiles.open"
    | "markdownFiles.save",
): void {
  const context = extensionContextForSender(sender);
  assertExtensionHostPermission({
    extensionId: context.id,
    source: context.source,
    manifestPermissions: context.manifestPermissions,
    capability,
  });
}

export function assertExtensionSenderHostPermissionIfExtension(
  sender: WebContents,
  capability:
    | "userData.read"
    | "userData.write"
    | "focus.control"
    | "network.read"
    | "dialogs.directory"
    | "markdownFiles.open"
    | "markdownFiles.save",
): void {
  const context = webContentsToExtensionContext.get(sender.id);
  if (!context) return;
  assertExtensionHostPermission({
    extensionId: context.id,
    source: context.source,
    manifestPermissions: context.manifestPermissions,
    capability,
  });
}

// ---------------------------------------------------------------------------
// IPC: list / open (kept from previous PoC contract)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", async (_e, id: string) => {
  await openExtension(id);
});
ipcMain.handle("kepler:eden-settings:open", async () => {
  await openExtension("eden", "#/settings", "eden:settings", "settings");
});
ipcMain.handle("kepler:eden-settings:close", (e) => {
  const key = extensionWindowKeyForSender(e.sender) ?? "eden:settings";
  const entry = extensionWindows.get(key);
  if (entry && !entry.win.isDestroyed()) {
    entry.win.close();
  }
});

registerExtensionArkIpc({
  contextForSender: extensionContextForSender,
});

registerExtensionWindowIpc({
  extensionIdForSender,
  initialRouteForSender,
  windowForSender,
});

registerExtensionImageColorIpc({ extensionIdForSender });
registerExtensionBookMetadataIpc({
  assertNetworkRead: (sender) => {
    const context = extensionContextForSender(sender);
    if (context.id !== "eden") {
      throw new Error("[kepler-shell] book metadata fetch is available only to Eden");
    }
    assertExtensionHostPermission({
      extensionId: context.id,
      source: context.source,
      manifestPermissions: context.manifestPermissions,
      capability: "network.read",
    });
  },
});

ipcMain.handle("kepler:extension:dialogs:pick-directory", async (event) => {
  assertExtensionSenderHostPermission(event.sender, "dialogs.directory");
  const parent = windowForSender(event.sender);
  const result = parent
    ? await dialog.showOpenDialog(parent, { properties: ["openDirectory"] })
    : await dialog.showOpenDialog({ properties: ["openDirectory"] });
  return result.canceled ? null : (result.filePaths[0] ?? null);
});

// ---------------------------------------------------------------------------
// IPC: host-action (extension → kepler-shell host action)
// ---------------------------------------------------------------------------

// Reserved для будущих host-action типа "show settings", "focus launcher" и т.п.
// Сейчас просто логирует и возвращает false (action not handled).
ipcMain.handle("kepler:extension:invoke-host", (_e, action: string, _payload?: JsonValue) => {
  console.error(`[kepler-shell] extension invoke-host: ${action} (no handler)`);
  return false;
});

registerExtensionMarkdownIpc({
  assertHostPermission: assertExtensionSenderHostPermission,
});

// ---------------------------------------------------------------------------
// IPC: userData (extension renderer → main → <APPDATA>/Kosmos/extensions-data/<id>/)
// ---------------------------------------------------------------------------
//
// Persistent user data extension'а — settings.json и любые другие files,
// которые extension хочет хранить локально (а не в ARK). Path физически
// отделён от code dir (`extensions/<id>/`), поэтому install/uninstall кода
// не трогает эти файлы. Каждый handler определяет extension id по sender —
// extension не может писать в чужой namespace.

function senderUserDataDir(sender: WebContents): string {
  const context = extensionContextForSender(sender);
  return ensureUserDataDir(context.id);
}

registerExtensionUserDataIpc({
  assertHostPermission: assertExtensionSenderHostPermission,
  userDataDirForSender: senderUserDataDir,
});

// ---------------------------------------------------------------------------
// IPC: .kext install / preview / list / revert / uninstall
// ---------------------------------------------------------------------------
//
// Используется install dialog (`#install-extension`) + Settings → Расширения.
// install:preview — читает .kext без extract'а, возвращает manifest preview;
// install:do — собственно установка с backup'ом;
// list — installed user-extensions для Settings UI;
// revert — восстановить из backup'а;
// uninstall — удалить user copy.

registerExtensionInstallerIpc();
registerMarketplaceIpc();
startPeriodicCatalogCheck();
