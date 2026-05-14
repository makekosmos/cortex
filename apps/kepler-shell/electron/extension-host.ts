import { BrowserWindow, ipcMain, screen } from "electron";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

export interface ExtensionManifest {
  id: string;
  name: string;
  entryHtml: string;
  preload?: string;
  width?: number;
  height?: number;
}

const extensionWindows = new Map<string, BrowserWindow>();

function resolveExtensionsRoot(): string {
  // dev: <repo>/apps/kepler-shell/extensions/
  // prod packaged: process.resourcesPath/extensions/
  const dev = path.resolve(__dirname, "..", "extensions");
  if (existsSync(dev)) return dev;
  return path.join(process.resourcesPath ?? __dirname, "extensions");
}

export function loadExtensionManifest(id: string): ExtensionManifest | null {
  const root = resolveExtensionsRoot();
  const manifestPath = path.join(root, id, "manifest.json");
  if (!existsSync(manifestPath)) return null;
  try {
    return JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
  } catch (e) {
    console.error(`[kepler-shell] extension manifest invalid: ${id}`, e);
    return null;
  }
}

export function listExtensions(): ExtensionManifest[] {
  const root = resolveExtensionsRoot();
  if (!existsSync(root)) return [];
  const out: ExtensionManifest[] = [];
  try {
    const entries = readdirSync(root, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory()) {
        const m = loadExtensionManifest(entry.name);
        if (m) out.push(m);
      }
    }
  } catch {
    /* ignore */
  }
  return out;
}

export function openExtension(id: string): void {
  const existing = extensionWindows.get(id);
  if (existing && !existing.isDestroyed()) {
    existing.focus();
    return;
  }
  const manifest = loadExtensionManifest(id);
  if (!manifest) {
    console.warn(`[kepler-shell] extension not found: ${id}`);
    return;
  }
  const root = resolveExtensionsRoot();
  const display = screen.getPrimaryDisplay().workAreaSize;
  const width = manifest.width ?? 900;
  const height = manifest.height ?? 600;
  const win = new BrowserWindow({
    width,
    height,
    x: Math.round((display.width - width) / 2),
    y: Math.round((display.height - height) / 2),
    show: true,
    title: manifest.name,
    backgroundColor: "#1a1a1a",
    webPreferences: {
      preload: manifest.preload
        ? path.join(root, id, manifest.preload)
        : undefined,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  void win.loadFile(path.join(root, id, manifest.entryHtml));
  win.on("closed", () => extensionWindows.delete(id));
  extensionWindows.set(id, win);
}

// Top-level IPC registration (side-effect import).
ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", (_e, id: string) => openExtension(id));
