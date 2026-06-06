import { BrowserWindow, clipboard, nativeImage, shell } from "electron";
import { execFileSync } from "node:child_process";
import path from "node:path";
import type {
  ClipboardHistoryItem,
  ClipboardHistorySettings,
  ClipboardHistorySettingsPatch,
  ClipboardHistoryStats,
} from "../shared/ipc-types";
import { createClipboardHistoryStore } from "./clipboard-history-store";
import { keplerDataDir } from "./data-dir";
import { safeHandle } from "./ipc-safe";

const CLIPBOARD_POLL_MS = 800;

interface ClipboardSourceInfo {
  name?: string;
  icon?: string;
}

const store = createClipboardHistoryStore({
  storagePath: path.join(keplerDataDir(), "clipboard-history.json"),
});
let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastSeenText = "";
let lastSeenImageDataUrl = "";
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
      lastSeenImageDataUrl = item.imageDataUrl;
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

function recordClipboardSnapshot(): void {
  let updated = false;
  const source = createClipboardSourceGetter();
  if (recordClipboardText(clipboard.readText(), source)) updated = true;
  if (recordClipboardFiles(source)) updated = true;
  if (recordClipboardImage(source)) updated = true;
  if (updated) broadcastUpdated();
}

function recordClipboardText(text: string, source: () => ClipboardSourceInfo): boolean {
  if (text === lastSeenText) return false;
  lastSeenText = text;
  const info = source();
  const item = store.record(text, { source: info.name, sourceIcon: info.icon });
  return !!item;
}

function recordClipboardFiles(source: () => ClipboardSourceInfo): boolean {
  const paths = readClipboardFilePaths();
  const fingerprint = paths.join("\n");
  if (fingerprint === lastSeenFilePaths) return false;
  lastSeenFilePaths = fingerprint;
  let updated = false;
  for (const filePath of paths) {
    const info = source();
    if (store.recordFile(filePath, { source: info.name, sourceIcon: info.icon })) updated = true;
  }
  return updated;
}

function recordClipboardImage(source: () => ClipboardSourceInfo): boolean {
  const image = clipboard.readImage();
  if (image.isEmpty()) return false;
  const dataUrl = image.toDataURL();
  if (!dataUrl || dataUrl === lastSeenImageDataUrl) return false;
  lastSeenImageDataUrl = dataUrl;
  const size = image.getSize();
  const info = source();
  const item = store.recordImage({
    dataUrl,
    width: size.width,
    height: size.height,
    mimeType: "image/png",
    source: info.name,
    sourceIcon: info.icon,
  });
  return !!item;
}

function createClipboardSourceGetter(): () => ClipboardSourceInfo {
  let resolved = false;
  let value: ClipboardSourceInfo = {};
  return () => {
    if (!resolved) {
      value = detectClipboardSource();
      resolved = true;
    }
    return value;
  };
}

function detectClipboardSource(): ClipboardSourceInfo {
  if (process.platform !== "win32") return {};
  try {
    const output = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        String.raw`
Add-Type -Namespace Kosmos -Name Win32 -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern System.IntPtr GetClipboardOwner();
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern uint GetWindowThreadProcessId(System.IntPtr hWnd, out uint processId);
'@
$hwnd = [Kosmos.Win32]::GetClipboardOwner()
if ($hwnd -eq [System.IntPtr]::Zero) { exit 0 }
$processId = 0
[void][Kosmos.Win32]::GetWindowThreadProcessId($hwnd, [ref]$processId)
if ($processId -eq 0) { exit 0 }
$p = Get-Process -Id $processId -ErrorAction SilentlyContinue
if ($null -eq $p) { exit 0 }
$icon = $null
if ($p.Path) {
  try {
    Add-Type -AssemblyName System.Drawing
    $extracted = [System.Drawing.Icon]::ExtractAssociatedIcon($p.Path)
    if ($null -ne $extracted) {
      $bitmap = $extracted.ToBitmap()
      $stream = [System.IO.MemoryStream]::new()
      $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
      $icon = "data:image/png;base64," + [Convert]::ToBase64String($stream.ToArray())
      $stream.Dispose()
      $bitmap.Dispose()
      $extracted.Dispose()
    }
  } catch {}
}
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
@{ name = $p.ProcessName; icon = $icon } | ConvertTo-Json -Compress
`,
      ],
      { encoding: "utf8", timeout: 500, windowsHide: true },
    );
    const raw = output.trim();
    if (!raw) return {};
    const parsed = JSON.parse(raw) as { name?: unknown; icon?: unknown };
    return {
      name: typeof parsed.name === "string" && parsed.name.trim() ? parsed.name.trim() : undefined,
      icon:
        typeof parsed.icon === "string" && parsed.icon.startsWith("data:image/")
          ? parsed.icon
          : undefined,
    };
  } catch {
    return {};
  }
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
