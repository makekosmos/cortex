import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import fs from 'node:fs';

const isDev = !app.isPackaged;

function getDataDir(): string {
  const dir = path.join(app.getPath('appData'), 'delphi', 'data');
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function createWindow() {
  const win = new BrowserWindow({
    width: 800,
    height: 600,
    minWidth: 400,
    minHeight: 200,
    title: 'Delphi',
    titleBarStyle: 'hiddenInset',
    webPreferences: {
      preload: path.join(__dirname, 'preload.mjs'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    win.loadFile(path.join(__dirname, '../dist/index.html'));
  }
}

// --- IPC handlers for filesystem storage ---

ipcMain.handle('fs:exists', async (_event, filePath: string) => {
  const fullPath = path.join(getDataDir(), filePath);
  return fs.existsSync(fullPath);
});

ipcMain.handle('fs:readTextFile', async (_event, filePath: string) => {
  const fullPath = path.join(getDataDir(), filePath);
  return fs.readFileSync(fullPath, 'utf-8');
});

ipcMain.handle(
  'fs:writeTextFile',
  async (_event, filePath: string, contents: string) => {
    const fullPath = path.join(getDataDir(), filePath);
    const dir = path.dirname(fullPath);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(fullPath, contents, 'utf-8');
  },
);

ipcMain.handle('fs:mkdir', async (_event, dirPath: string) => {
  const fullPath = path.join(getDataDir(), dirPath);
  fs.mkdirSync(fullPath, { recursive: true });
});

// --- App lifecycle ---

app.whenReady().then(createWindow);

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
