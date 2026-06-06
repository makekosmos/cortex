import { BrowserWindow, clipboard, nativeImage, shell, type NativeImage } from "electron";
import path from "node:path";
import type {
  ClipboardHistoryItem,
  ClipboardHistorySettings,
  ClipboardHistorySettingsPatch,
  ClipboardHistoryStats,
} from "../shared/ipc-types";
import { createClipboardHistoryStore, fingerprintImageBytes } from "./clipboard-history-store";
import { keplerDataDir } from "./data-dir";
import { safeHandle } from "./ipc-safe";

const CLIPBOARD_POLL_MS = 800;

const store = createClipboardHistoryStore({
  storagePath: path.join(keplerDataDir(), "clipboard-history.json"),
});
let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastSeenText = "";
let lastSeenImageFingerprint = "";
let lastSeenFilePaths = "";
let registered = false;
let shellOpener: (() => void) | null = null;

export function setClipboardHistoryShellOpener(opener: () => void): void {
  shellOpener = opener;
}

export function openClipboardHistoryShell(): void {
  if (shellOpener) {
    shellOpener();
    return;
  }
  console.warn("[clipboard-history] shell opener is not registered");
}

export function startClipboardHistory(): void {
  if (pollTimer) return;
  store.pruneNow();
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
      lastSeenImageFingerprint = imageFingerprint(clipboard.readImage());
      store.recordImage({
        dataUrl: item.imageDataUrl,
        width: item.width ?? image.getSize().width,
        height: item.height ?? image.getSize().height,
        mimeType: item.mimeType,
        source: item.source,
        sourceIcon: item.sourceIcon,
      });
    } else if (item.kind === "file" && item.filePath) {
      clipboard.writeText(item.filePath);
      lastSeenText = item.filePath;
      store.updateTimestamp(item.id);
    } else {
      clipboard.writeText(item.text);
      lastSeenText = item.text;
      store.updateTimestamp(item.id);
    }
    broadcastUpdated();
    return true;
  });
  safeHandle("kepler:clipboard-history:open", async (_event, id: string): Promise<boolean> => {
    const item = store.find(id);
    if (!item) return false;
    if (item.kind === "link" && item.url) {
      await shell.openExternal(item.url);
      return true;
    }
    if (item.kind === "file" && item.filePath) {
      const error = await shell.openPath(item.filePath);
      return !error;
    }
    return false;
  });
  safeHandle(
    "kepler:clipboard-history:toggle-pin",
    async (_event, id: string): Promise<boolean> => {
      const item = store.togglePin(id);
      if (!item) return false;
      broadcastUpdated();
      return true;
    },
  );
  safeHandle("kepler:clipboard-history:delete", async (_event, id: string): Promise<boolean> => {
    const removed = store.remove(id);
    if (removed) broadcastUpdated();
    return removed;
  });
  safeHandle("kepler:clipboard-history:clear", async (): Promise<void> => {
    store.clear();
    broadcastUpdated();
  });
  safeHandle("kepler:clipboard-history:clear-all", async (): Promise<void> => {
    store.clearAll();
    broadcastUpdated();
  });
  safeHandle("kepler:clipboard-history:settings", async (): Promise<ClipboardHistorySettings> => {
    return store.settings();
  });
  safeHandle(
    "kepler:clipboard-history:settings:update",
    async (_event, patch: ClipboardHistorySettingsPatch): Promise<ClipboardHistorySettings> => {
      const settings = store.updateSettings(patch);
      broadcastUpdated();
      return settings;
    },
  );
  safeHandle("kepler:clipboard-history:stats", async (): Promise<ClipboardHistoryStats> => {
    return store.stats();
  });
  safeHandle("kepler:clipboard-history:hide", async (): Promise<void> => {});
}

function imageFingerprint(image: NativeImage): string {
  const size = image.getSize();
  return fingerprintImageBytes(size.width, size.height, image.toBitmap());
}

/**
 * Снимок буфера обмена — полностью синхронный и дешёвый: только чтение текста /
 * файлов / картинки. Никаких внешних процессов в горячем пути (раньше на каждое
 * копирование синхронно поднимался `powershell.exe` + `Add-Type` для определения
 * приложения-источника и вешал ввод). PNG-кодирование картинки (`toDataURL`)
 * выполняется только при смене отпечатка raw-битмапа.
 */
function recordClipboardSnapshot(): void {
  const text = clipboard.readText();
  const textChanged = text !== lastSeenText;

  const filePaths = readClipboardFilePaths();
  const filesFingerprint = filePaths.join("\n");
  const filesChanged = filesFingerprint !== lastSeenFilePaths;

  const image = clipboard.readImage();
  const hasImage = !image.isEmpty();
  const imgFingerprint = hasImage ? imageFingerprint(image) : "";
  const imageChanged = hasImage && imgFingerprint !== lastSeenImageFingerprint;

  if (!textChanged && !filesChanged && !imageChanged) return;

  let updated = false;

  if (textChanged) {
    lastSeenText = text;
    if (store.record(text)) updated = true;
  }
  if (filesChanged) {
    lastSeenFilePaths = filesFingerprint;
    for (const filePath of filePaths) {
      if (store.recordFile(filePath)) updated = true;
    }
  }
  if (imageChanged) {
    lastSeenImageFingerprint = imgFingerprint;
    const dataUrl = image.toDataURL();
    if (dataUrl) {
      const size = image.getSize();
      if (
        store.recordImage({
          dataUrl,
          width: size.width,
          height: size.height,
          mimeType: "image/png",
        })
      ) {
        updated = true;
      }
    }
  }

  if (updated) broadcastUpdated();
}

function readClipboardFilePaths(): string[] {
  const formats = clipboard.availableFormats();
  const fromUriList = formats.includes("text/uri-list")
    ? parseUriList(clipboard.readBuffer("text/uri-list").toString("utf8"))
    : [];
  const fromFileNameW = formats.includes("FileNameW")
    ? parseWindowsFileNameBuffer(clipboard.readBuffer("FileNameW"))
    : [];
  const fromFileName = formats.includes("FileName")
    ? clipboard
        .readBuffer("FileName")
        .toString("utf8")
        .split("\0")
        .map((item) => item.trim())
        .filter(Boolean)
    : [];

  return [...new Set([...fromUriList, ...fromFileNameW, ...fromFileName])];
}

function parseUriList(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"))
    .map((line) => {
      try {
        const url = new URL(line);
        return url.protocol === "file:" ? decodeURIComponent(url.pathname) : "";
      } catch {
        return "";
      }
    })
    .map((line) => line.replace(/^\/([A-Za-z]:\/)/, "$1").replace(/\//g, "\\"))
    .filter(Boolean);
}

function parseWindowsFileNameBuffer(buffer: Buffer): string[] {
  if (buffer.byteLength === 0) return [];
  return buffer
    .toString("utf16le")
    .split("\0")
    .map((item) => item.trim())
    .filter(Boolean);
}

function broadcastUpdated(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send("kepler:clipboard-history:updated");
    }
  }
}
