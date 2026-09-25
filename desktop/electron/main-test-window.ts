import { BrowserWindow } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Headless harness window для Playwright e2e: единственное окно, которое
// desktop shell открывает в KOSMOS_TEST_MODE. Грузит renderer bundle без
// hash (src/main.ts рендерит пустой stub) — тестам нужен только
// `window.kepler` preload bridge, не UI.
export function openTestHarnessWindow(): void {
  const win = new BrowserWindow({
    width: 720,
    height: 460,
    show: false,
    skipTaskbar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  const devServerUrl = process.env.VITE_DEV_SERVER_URL;
  if (devServerUrl) {
    void win.loadURL(devServerUrl);
  } else {
    void win.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}
