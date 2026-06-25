import { BrowserWindow, ipcMain, screen, type WebContents } from "electron";

type ExtensionWindowBounds = { x: number; y: number; width: number; height: number };

type ExtensionWindowDragState = {
  startBounds: ExtensionWindowBounds;
  startScreenX: number;
  startScreenY: number;
};

interface ExtensionWindowIpcOptions {
  extensionIdForSender(sender: WebContents): string | null;
  initialRouteForSender(sender: WebContents): string | null;
  windowForSender(sender: WebContents): BrowserWindow | null;
}

const extensionWindowDrags = new Map<number, ExtensionWindowDragState>();
const extensionTitlebarHoverTrackers = new Map<number, NodeJS.Timeout>();
const dockedState = new Map<
  string,
  { x: number; y: number; width: number; height: number; alwaysOnTop: boolean }
>();

const DOCK_WIDTH = 360;
const DOCK_HEIGHT = 560;

function clearExtensionTitlebarHoverTracker(webContentsId: number): void {
  const interval = extensionTitlebarHoverTrackers.get(webContentsId);
  if (!interval) return;
  clearInterval(interval);
  extensionTitlebarHoverTrackers.delete(webContentsId);
}

function broadcastDocked(win: BrowserWindow, isDocked: boolean): void {
  if (win.isDestroyed()) return;
  try {
    win.webContents.send("kepler:extension:window:docked-changed", isDocked);
  } catch {
    // renderer not ready, ignore
  }
}

export function clearExtensionWindowIpcState(webContentsId: number): void {
  clearExtensionTitlebarHoverTracker(webContentsId);
  extensionWindowDrags.delete(webContentsId);
}

export function registerExtensionWindowIpc({
  extensionIdForSender,
  initialRouteForSender,
  windowForSender,
}: ExtensionWindowIpcOptions): void {
  ipcMain.handle("kepler:extension:meta:id", (e) => extensionIdForSender(e.sender));

  ipcMain.handle("kepler:extension:navigation:initial", (e): string | null => {
    return initialRouteForSender(e.sender);
  });

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

  ipcMain.handle("kepler:extension:window:is-maximized", (e): boolean => {
    const win = windowForSender(e.sender);
    return win ? win.isMaximized() : false;
  });

  ipcMain.handle("kepler:extension:window:zoom-get", (e): number => {
    return e.sender.getZoomFactor();
  });

  ipcMain.handle("kepler:extension:window:zoom-set", (e, factor: number): number => {
    const next = typeof factor === "number" && Number.isFinite(factor) ? factor : 1;
    const clamped = Math.max(0.5, Math.min(2.0, next));
    e.sender.setZoomFactor(clamped);
    return clamped;
  });

  ipcMain.handle("kepler:extension:window:set-maximizable", (e, value: boolean) => {
    const win = windowForSender(e.sender);
    if (!win || win.isDestroyed()) return;
    win.setMaximizable(Boolean(value));
  });

  ipcMain.handle("kepler:extension:window:set-titlebar-symbol-color", (e, symbolColor: string) => {
    const win = windowForSender(e.sender);
    if (!win || win.isDestroyed()) return;
    win.setTitleBarOverlay({
      color: "#00000000",
      symbolColor,
      height: 40,
    });
  });

  ipcMain.handle("kepler:extension:window:begin-manual-drag", (e, point) => {
    const win = windowForSender(e.sender);
    if (!win || win.isDestroyed()) return;
    if (!point || typeof point.screenX !== "number" || typeof point.screenY !== "number") return;

    if (win.isMaximized()) win.unmaximize();
    extensionWindowDrags.set(e.sender.id, {
      startBounds: win.getBounds(),
      startScreenX: point.screenX,
      startScreenY: point.screenY,
    });
  });

  ipcMain.handle("kepler:extension:window:move-manual-drag", (e, point) => {
    const win = windowForSender(e.sender);
    const drag = extensionWindowDrags.get(e.sender.id);
    if (!win || win.isDestroyed() || !drag) return;
    if (!point || typeof point.screenX !== "number" || typeof point.screenY !== "number") return;

    win.setBounds({
      ...drag.startBounds,
      x: Math.round(drag.startBounds.x + point.screenX - drag.startScreenX),
      y: Math.round(drag.startBounds.y + point.screenY - drag.startScreenY),
    });
  });

  ipcMain.handle("kepler:extension:window:end-manual-drag", (e) => {
    extensionWindowDrags.delete(e.sender.id);
  });

  ipcMain.handle("kepler:extension:window:set-titlebar-hover-tracking", (e, enabled, height) => {
    const win = windowForSender(e.sender);
    if (!win || win.isDestroyed()) return;

    const webContentsId = e.sender.id;
    clearExtensionTitlebarHoverTracker(webContentsId);

    if (!enabled) {
      e.sender.send("kepler:extension:window:titlebar-hover-changed", false);
      return;
    }

    const titlebarHeight = typeof height === "number" && height > 0 ? height : 56;
    let lastHovered: boolean | null = null;

    const interval = setInterval(() => {
      if (win.isDestroyed() || e.sender.isDestroyed()) {
        clearExtensionTitlebarHoverTracker(webContentsId);
        return;
      }

      const cursor = screen.getCursorScreenPoint();
      const bounds = win.getBounds();
      const hovered =
        cursor.x >= bounds.x &&
        cursor.x <= bounds.x + bounds.width &&
        cursor.y >= bounds.y &&
        cursor.y <= bounds.y + titlebarHeight;

      if (hovered === lastHovered) return;
      lastHovered = hovered;
      e.sender.send("kepler:extension:window:titlebar-hover-changed", hovered);
    }, 33);

    extensionTitlebarHoverTrackers.set(webContentsId, interval);
  });

  ipcMain.handle("kepler:extension:window:toggle-dock-corner", (e) => {
    const id = extensionIdForSender(e.sender);
    const win = windowForSender(e.sender);
    if (!id || !win || win.isDestroyed()) return;

    const stored = dockedState.get(id);
    if (stored) {
      win.setAlwaysOnTop(stored.alwaysOnTop);
      win.setBounds({
        x: stored.x,
        y: stored.y,
        width: stored.width,
        height: stored.height,
      });
      dockedState.delete(id);
      broadcastDocked(win, false);
      return;
    }

    const current = win.getNormalBounds();
    dockedState.set(id, {
      x: current.x,
      y: current.y,
      width: current.width,
      height: current.height,
      alwaysOnTop: win.isAlwaysOnTop(),
    });

    if (win.isMaximized()) win.unmaximize();
    const workArea = screen.getDisplayMatching(current).workArea;
    const marginX = 12;
    const marginTop = 12;
    const width = Math.min(DOCK_WIDTH, workArea.width - marginX * 2);
    const height = Math.min(DOCK_HEIGHT, workArea.height - marginTop);
    win.setBounds({
      x: workArea.x + workArea.width - width - marginX,
      y: workArea.y + marginTop,
      width,
      height,
    });

    setTimeout(() => {
      if (win.isDestroyed()) return;
      const actual = win.getBounds();
      const overflow = actual.x + actual.width - (workArea.x + workArea.width);
      if (overflow > 0) {
        win.setBounds({ ...actual, x: actual.x - overflow - marginX });
      }
    }, 0);

    win.setAlwaysOnTop(true, "floating");
    broadcastDocked(win, true);
  });

  ipcMain.handle("kepler:extension:window:is-docked", (e): boolean => {
    const id = extensionIdForSender(e.sender);
    return id ? dockedState.has(id) : false;
  });
}
