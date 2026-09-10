import { BrowserWindow, globalShortcut, screen } from "electron";
import path from "node:path";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  type KosmosWindowMaterial,
} from "./window-effects";
import { openHostedApp } from "./host-app";
import { openManager } from "./manager-navigation";

interface LauncherPosition {
  x: number;
  y: number;
}

interface RegisterLauncherHotkeysOptions {
  slot: string;
  slotHotkey: string | null;
  getStoredHotkey: () => string;
  normalizeHotkeyAccelerator: (accelerator: string) => string | null;
  setHotkeyReregisterCallback: (callback: (accelerator: string) => boolean) => void;
}

interface LauncherControllerOptions {
  isDev: boolean;
  productName: string;
  windowWidth: number;
  windowHeight: number;
  dirname: string;
  devServerUrl?: string;
  resolveBackgroundMaterial: () => KosmosWindowMaterial;
  getIsQuiting: () => boolean;
  openSettings: () => void;
  quitApplication: () => void;
  onLauncherShow?: () => void;
}

export interface LauncherController {
  createLauncher(): void;
  showLauncher(): void;
  hideLauncher(): void;
  showFocusSessionLauncher(): void;
  setLauncherExpanded(expanded: boolean): void;
  setTrayVisible(enabled: boolean): void;
  openManager(): void;
  registerLauncherHotkeys(options: RegisterLauncherHotkeysOptions): void;
  getMainWindow(): BrowserWindow | null;
  isLauncherHidden(): boolean;
}

export function createLauncherController(options: LauncherControllerOptions): LauncherController {
  const {
    dirname,
    devServerUrl,
    getIsQuiting,
    isDev,
    onLauncherShow,
    resolveBackgroundMaterial,
    windowHeight,
    windowWidth,
  } = options;

  let mainWindow: BrowserWindow | null = null;
  let launcherHidden = true;

  function defaultLauncherPosition(): LauncherPosition {
    const display = screen.getPrimaryDisplay().workAreaSize;
    return {
      x: Math.round((display.width - windowWidth) / 2),
      y: Math.round(display.height * 0.15),
    };
  }

  function createLauncher(): void {
    const pos = defaultLauncherPosition();
    const backgroundMaterial = resolveBackgroundMaterial();

    mainWindow = new BrowserWindow({
      width: windowWidth,
      height: windowHeight,
      x: pos.x,
      y: pos.y,
      show: false,
      paintWhenInitiallyHidden: true,
      frame: false,
      // Acrylic / mica игнорируется при transparent:true. На Win11 22H2+ окно
      // автоматически получает rounded corners. Acrylic intense чем mica -
      // лучше визуально для launcher'а (как PowerToys Run / Raycast).
      transparent: false,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      // См. postmortems.md § 2026-06-07. На macOS frameless BrowserWindow с
      // always-on-top + showInactive/focus может активировать app без видимого
      // reactive window после первого input event. Этот path нужен только Windows.
      alwaysOnTop: process.platform === "win32",
      ...backgroundMaterialOption(backgroundMaterial),
      backgroundColor: "#1d1d1f",
      roundedCorners: true,
      webPreferences: {
        preload: path.join(dirname, "preload.mjs"),
        contextIsolation: true,
        nodeIntegration: false,
        // macOS occlusion throttling: frameless launcher без постоянного
        // always-on-top может заморозить paint. На Windows/Linux throttling
        // должен оставаться включенным для скрытого окна. См. postmortems.md
        // § 2026-06-07 и § 2026-06-08.
        backgroundThrottling: process.platform !== "darwin",
      },
    });

    // Явный вызов после create - иногда constructor option backgroundMaterial
    // не применяется на frameless+alwaysOnTop комбинации; setBackgroundMaterial
    // прямо дёргает DwmSetWindowAttribute. Безопасно: no-op на non-Win11.
    try {
      applyWindowMaterial(mainWindow, backgroundMaterial, "launcher");
    } catch {}

    // Hide launcher при потере фокуса (клик вне окна / Alt+Tab).
    // В dev пропускаем если фокус ушёл на DevTools - иначе нечем отлаживать.
    mainWindow.on("blur", () => {
      if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
      if (isDev && mainWindow?.webContents.isDevToolsFocused()) return;
      hideLauncher();
    });
    mainWindow.on("close", (e) => {
      if (!getIsQuiting() && process.env.KOSMOS_HEADLESS !== "1") {
        e.preventDefault();
        hideLauncher();
      }
    });
    if (isDev && devServerUrl) {
      void mainWindow.loadURL(devServerUrl);
      // detached DevTools - отдельное окно, не блокирует launcher.
      // activate: false - не отдаём фокус DevTools при открытии: иначе
      // DevTools берёт фокус через ~1-2с и триггерит blur -> hideLauncher.
      if (process.env.KOSMOS_DEVTOOLS === "1") {
        mainWindow.webContents.openDevTools({ mode: "detach", activate: false });
      }
    } else {
      void mainWindow.loadFile(path.join(dirname, "../dist/index.html"));
    }
  }

  function showLauncher(): void {
    if (!mainWindow) createLauncher();
    if (!mainWindow) return;
    const headless = process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
    const pos = defaultLauncherPosition();
    mainWindow.setBounds({
      x: pos.x,
      y: pos.y,
      width: windowWidth,
      height: windowHeight,
    });
    mainWindow.setIgnoreMouseEvents(false);
    mainWindow.setOpacity(1);
    // В headless / test mode окно НИКОГДА не показывается визуально - Playwright
    // работает через webContents без paint'а. Renderer всё равно получает
    // `kepler:window:show` для focus/refresh, и `launcherHidden` обновляется.
    if (headless) {
      // Playwright attaches to a visible Electron target. This branch is only
      // used by isolated headless/test processes, never by the user build.
      mainWindow.show();
    } else {
      if (process.platform === "darwin") {
        // show() на macOS уже вызывает activateIgnoringOtherApps внутри Electron.
        // Не добавляем app.focus({ steal: true }) - двойной activate создавал
        // "войну фокуса" с предыдущим приложением -> blur через 1-2с -> hideLauncher.
        // См. postmortems.md § 2026-06-07.
        mainWindow.show();
        mainWindow.focus();
      } else {
        // Windows/Linux. Launcher открывается по globalShortcut, поэтому Kepler
        // shell - НЕ foreground-процесс: showInactive() показывал окно без
        // активации, а .focus() блокировался Windows foreground lock'ом (окно
        // всплывало поверх всего благодаря alwaysOnTop, но клавиатурный ввод
        // оставался в предыдущем приложении). show() активирует окно и отдаёт
        // ему фокус ввода - штатное поведение launcher'а (PowerToys Run / Raycast).
        mainWindow.show();
        mainWindow.focus();
        // Windows может всё равно отказать SetForegroundWindow фоновому процессу.
        // Кратковременный toggle alwaysOnTop (off->on) дёргает SetWindowPos с
        // HWND_TOPMOST и пинает систему реально вытащить окно на передний план.
        if (process.platform === "win32") {
          mainWindow.setAlwaysOnTop(false);
          mainWindow.setAlwaysOnTop(true);
          mainWindow.focus();
        }
      }
    }
    launcherHidden = false;
    mainWindow.webContents.send("kepler:window:show");
    onLauncherShow?.();
  }

  function showFocusSessionLauncher(): void {
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
    void openHostedApp("com.kosmos.shell");
  }

  function hideLauncher(): void {
    if (!mainWindow || mainWindow.isDestroyed()) return;
    if (launcherHidden) return;
    launcherHidden = true;
    // Раньше делали `setOpacity(0) + setIgnoreMouseEvents(true)` - это
    // визуально "прятало" окно, но Win32 EnumWindows / GetWindowList всё
    // ещё видели его как visible top-level window. Скриншот-апы (Snipping
    // Tool, ShareX) ловили пустой прямоугольник в кадр; Raycast/PowerToys
    // фильтруют по `IsWindowVisible` (= ShowWindow state) и не видели -
    // отсюда асимметрия. Настоящий `hide()` вызывает `ShowWindow(SW_HIDE)`,
    // окно уходит из enum'а для всех инструментов.
    mainWindow.webContents.send("kepler:window:hide");
    mainWindow.hide();
  }

  function setLauncherExpanded(_expanded: boolean): void {
    // Окно теперь fixed-size; IPC оставлен как noop для backward-compat.
  }

  function setTrayVisible(_enabled: boolean): void {
    // The backend runtime owns the single Windows tray icon.
  }

  function registerLauncherHotkeys({
    getStoredHotkey,
    normalizeHotkeyAccelerator,
    setHotkeyReregisterCallback,
    slot,
    slotHotkey,
  }: RegisterLauncherHotkeysOptions): void {
    // Anti-repeat по delta-времени между fire'ами. Windows key-repeat шлёт
    // WM_HOTKEY каждые ~33 мс пока сочетание зажато; тап-тап (с реальным
    // отпусканием пробела) даёт паузу >>=100 мс. Threshold 80 мс отрезает
    // auto-repeat но пропускает быстрые тапы (>12 Hz всё равно бывает редко).
    // Глобальный accelerator при этом всегда зарегистрирован - Windows не
    // выдаёт Alt+Space в системные меню окна.
    let lastFireAt = 0;
    const showHide = () => {
      const now = Date.now();
      const gap = now - lastFireAt;
      lastFireAt = now;
      if (gap < 80) return;
      if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
      void openHostedApp("com.kosmos.shell");
    };

    // Slot'ы без hotkey (dev-<x>, test-<x>) - пропускаем регистрацию вовсе.
    // Пользователь активирует launcher через tray click. Это критично для
    // multi-dev: два инстанса не могут поделить один accelerator, второй
    // молча проиграл бы Windows OS race.
    if (slotHotkey !== null) {
      let currentAccelerator = getStoredHotkey();
      function tryRegister(accelerator: string): boolean {
        const normalized = normalizeHotkeyAccelerator(accelerator);
        if (!normalized) return false;
        try {
          if (globalShortcut.isRegistered(currentAccelerator)) {
            globalShortcut.unregister(currentAccelerator);
          }
          const registered = globalShortcut.register(normalized, showHide);
          if (registered) {
            currentAccelerator = normalized;
            console.log(`[kepler-shell] globalShortcut ${normalized} registered`);
            return true;
          }
          // Откатываемся на предыдущий, если новая регистрация не удалась.
          globalShortcut.register(currentAccelerator, showHide);
          return false;
        } catch (e) {
          console.error("[kepler-shell] globalShortcut register error:", e);
          return false;
        }
      }
      const ok = tryRegister(currentAccelerator);
      setHotkeyReregisterCallback(tryRegister);
      if (!ok) {
        console.error(`[kepler-shell] globalShortcut ${currentAccelerator} register failed`);
      }
    } else {
      console.log(`[kepler-shell] hotkey disabled for slot ${slot} - use tray click`);
    }
    // F12 toggle DevTools (dev mode только) - глобальный hotkey удобнее чем
    // accelerator menu, т.к. меню у frameless окна нет. F12 не конфликтует
    // между параллельными dev-инстансами потому что Windows route'ит global
    // accelerator к одному фокусному окну; в multi-dev только активное окно
    // получит toggle, остальные тихо ничего не делают.
    if (isDev) {
      globalShortcut.register("F12", () => {
        mainWindow?.webContents.toggleDevTools();
      });
    }
  }

  return {
    createLauncher,
    showLauncher,
    hideLauncher,
    showFocusSessionLauncher,
    setLauncherExpanded,
    setTrayVisible,
    openManager,
    registerLauncherHotkeys,
    getMainWindow: () => mainWindow,
    isLauncherHidden: () => launcherHidden,
  };
}
