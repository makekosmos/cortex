// Install Extension dialog window — открывается при двойном клике .kext в
// проводнике (через argv / second-instance), либо из Settings → Расширения
// → «Установить из файла…».
//
// Window грузит renderer-bundle с hash `#install-extension`. Renderer
// (`src/views/InstallExtensionView.vue`) запрашивает preview через
// `window.kepler.extension.installPreview(path)` и показывает Confirm / Cancel.

import { app, BrowserWindow, screen } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
} from "./window-effects";
import { isHeadlessOrTest } from "./extension-manifest";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 520;
const WINDOW_HEIGHT = 480;

let installWindow: BrowserWindow | null = null;

export function openInstallExtensionWindow(sourcePath: string): void {
  if (installWindow && !installWindow.isDestroyed()) {
    // Если уже открыто — просто заменяем path query и фокусируем.
    if (!isHeadlessOrTest()) installWindow.focus();
    void installWindow.webContents.send("kepler:extension:install:source-changed", sourcePath);
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;
  const backgroundMaterial = resolveWindowMaterial("none");
  installWindow = new BrowserWindow({
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
    x: Math.round((display.width - WINDOW_WIDTH) / 2),
    y: Math.round((display.height - WINDOW_HEIGHT) / 2),
    show: !isHeadlessOrTest(),
    frame: false,
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: isHeadlessOrTest(),
    alwaysOnTop: false,
    backgroundColor: "#1a1a1a",
    ...backgroundMaterialOption(backgroundMaterial),
    roundedCorners: true,
    title: "CosCast — Установка расширения",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
      // Передаём sourcePath через additionalArguments — renderer читает
      // его при mount'е, не нужно дополнительного round-trip через IPC.
      additionalArguments: [`--kext-source=${sourcePath}`],
    },
  });

  try {
    applyWindowMaterial(installWindow, backgroundMaterial, "install-extension");
  } catch {
    /* non-Win11 ignored */
  }

  const hash = `install-extension?path=${encodeURIComponent(sourcePath)}`;
  if (process.env.VITE_DEV_SERVER_URL) {
    void installWindow.loadURL(`${process.env.VITE_DEV_SERVER_URL}#${hash}`);
  } else {
    void installWindow.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash,
    });
  }

  installWindow.on("closed", () => {
    installWindow = null;
  });
}

/**
 * Сканирует process.argv на .kext путь. Возвращает абсолютный path или null.
 *
 * Поддерживает:
 *   - `Kepler.exe path\to\extension.kext` (Windows file association дефолт);
 *   - `Kepler.exe --ext-install path\to\extension.kext` (explicit flag для CLI).
 */
export function findKextInArgv(argv: string[]): string | null {
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--ext-install" && i + 1 < argv.length) {
      return path.resolve(argv[i + 1]!);
    }
    if (a.startsWith("--ext-install=")) {
      return path.resolve(a.slice("--ext-install=".length));
    }
    // Electron в packaged режиме передаёт main.js как первый аргумент
    // (если запущено напрямую через `electron .`), либо сразу
    // дополнительные аргументы. .kext-путь распознаём по расширению.
    if (a.toLowerCase().endsWith(".kext") && !a.startsWith("-")) {
      // Не main.js / dist-electron — иначе это электроновский внутренний.
      if (!a.includes("dist-electron") && !a.endsWith("main.js")) {
        try {
          // Только если файл реально существует — argv может содержать
          // мусор от Electron-runtime'а.
          // (require fs inline'ом чтобы не тащить top-level import.)
          // eslint-disable-next-line @typescript-eslint/no-require-imports
          // SAFETY: the built-in Node module exposes the exact runtime API required here.
          const fs = require("node:fs") as typeof import("node:fs");
          if (fs.existsSync(a)) return path.resolve(a);
        } catch {
          /* ignore */
        }
      }
    }
  }
  return null;
}

void app; // suppress unused import warning when reorganizing
