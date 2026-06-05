import { BrowserWindow, clipboard, nativeImage, screen } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { ClipboardHistoryItem } from "../shared/ipc-types";
import { createClipboardHistoryStore } from "./clipboard-history-store";
import { safeHandle } from "./ipc-safe";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const CLIPBOARD_WINDOW_WIDTH = 760;
const CLIPBOARD_WINDOW_HEIGHT = 520;
const CLIPBOARD_WINDOW_MIN_WIDTH = 560;
const CLIPBOARD_WINDOW_MIN_HEIGHT = 380;
const CLIPBOARD_POLL_MS = 800;

const store = createClipboardHistoryStore();
let clipboardWin: BrowserWindow | null = null;
let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastSeenText = "";
let lastSeenImageDataUrl = "";
let registered = false;
let shellOpener: (() => void) | null = null;

function isHeadlessOrTest(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
}

export function openClipboardHistoryWindow(): void {
  if (clipboardWin && !clipboardWin.isDestroyed()) {
    if (isHeadlessOrTest()) return;
    clipboardWin.show();
    clipboardWin.focus();
    clipboardWin.webContents.send("kepler:clipboard-history:updated");
    return;
  }

  const workArea = screen.getPrimaryDisplay().workArea;
  const x = Math.round(workArea.x + (workArea.width - CLIPBOARD_WINDOW_WIDTH) / 2);
  const y = Math.round(workArea.y + (workArea.height - CLIPBOARD_WINDOW_HEIGHT) / 2);

  clipboardWin = new BrowserWindow({
    width: CLIPBOARD_WINDOW_WIDTH,
    height: CLIPBOARD_WINDOW_HEIGHT,
    minWidth: CLIPBOARD_WINDOW_MIN_WIDTH,
    minHeight: CLIPBOARD_WINDOW_MIN_HEIGHT,
    x,
    y,
    show: !isHeadlessOrTest(),
    title: "Буфер обмена",
    backgroundColor: "#0d0d0d",
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#FFFFFF",
      height: 36,
    },
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  clipboardWin.webContents.on("before-input-event", (event, input) => {
    if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
      event.preventDefault();
      try {
        clipboardWin?.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight */
      }
    }
  });

  clipboardWin.on("closed", () => {
    clipboardWin = null;
  });

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void clipboardWin.loadURL(`${devUrl}#clipboard-history`);
  } else {
    void clipboardWin.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash: "clipboard-history",
    });
  }
}

export function setClipboardHistoryShellOpener(opener: () => void): void {
  shellOpener = opener;
}

export function openClipboardHistoryShell(): void {
  if (shellOpener) {
    shellOpener();
    return;
  }
  openClipboardHistoryWindow();
}

export function startClipboardHistory(): void {
  if (pollTimer) return;
  recordClipboardSnapshot();
  pollTimer = setInterval(() => {
    recordClipboardSnapshot();
  }, CLIPBOARD_POLL_MS);
}

export function stopClipboardHistory(): void {
  if (!pollTimer) return;
  clearInterval(pollTimer);
  pollTimer = null;
}

export function registerClipboardHistoryIpc(): void {
  if (registered) return;
  registered = true;

  safeHandle("kepler:clipboard-history:list", async (): Promise<ClipboardHistoryItem[]> => {
    return store.list();
  });
  safeHandle("kepler:clipboard-history:copy", async (_event, id: string): Promise<boolean> => {
    const item = store.find(id);
    if (!item) return false;
    if (item.kind === "image" && item.imageDataUrl) {
      const image = nativeImage.createFromDataURL(item.imageDataUrl);
      if (image.isEmpty()) return false;
      clipboard.writeImage(image);
      lastSeenImageDataUrl = item.imageDataUrl;
      store.recordImage({
        dataUrl: item.imageDataUrl,
        width: item.width ?? image.getSize().width,
        height: item.height ?? image.getSize().height,
      });
    } else {
      clipboard.writeText(item.text);
      lastSeenText = item.text;
      store.record(item.text);
    }
    broadcastUpdated();
    return true;
  });
  safeHandle("kepler:clipboard-history:delete", async (_event, id: string): Promise<boolean> => {
    const removed = store.remove(id);
    if (removed) broadcastUpdated();
    return removed;
  });
  safeHandle("kepler:clipboard-history:clear", async (): Promise<void> => {
    store.clear();
    broadcastUpdated();
  });
  safeHandle("kepler:clipboard-history:hide", async (): Promise<void> => {
    clipboardWin?.hide();
  });
}

function recordClipboardSnapshot(): void {
  let updated = false;
  if (recordClipboardText(clipboard.readText())) updated = true;
  if (recordClipboardImage()) updated = true;
  if (updated) broadcastUpdated();
}

function recordClipboardText(text: string): boolean {
  if (text === lastSeenText) return false;
  lastSeenText = text;
  const item = store.record(text);
  return !!item;
}

function recordClipboardImage(): boolean {
  const image = clipboard.readImage();
  if (image.isEmpty()) return false;
  const dataUrl = image.toDataURL();
  if (!dataUrl || dataUrl === lastSeenImageDataUrl) return false;
  lastSeenImageDataUrl = dataUrl;
  const size = image.getSize();
  const item = store.recordImage({
    dataUrl,
    width: size.width,
    height: size.height,
  });
  return !!item;
}

function broadcastUpdated(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send("kepler:clipboard-history:updated");
    }
  }
}
