import { BrowserWindow, globalShortcut, Menu, nativeImage, screen, Tray } from "electron";
import path from "node:path";
import { existsSync } from "node:fs";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  type KosmosWindowMaterial,
} from "./window-effects";

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
  showClipboardHistoryLauncher(): void;
  showFocusSessionLauncher(): void;
  setLauncherExpanded(expanded: boolean): void;
  setTrayVisible(enabled: boolean): void;
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
    openSettings,
    productName,
    quitApplication,
    resolveBackgroundMaterial,
    windowHeight,
    windowWidth,
  } = options;

  let mainWindow: BrowserWindow | null = null;
  let tray: Tray | null = null;
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
      backgroundColor: "#00000000",
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
      if (isDev && mainWindow?.webContents.isDevToolsFocused()) return;
      hideLauncher();
    });
    mainWindow.on("close", (e) => {
      if (!getIsQuiting()) {
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
    if (!headless) {
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

  function showClipboardHistoryLauncher(): void {
    showLauncher();
    if (!mainWindow || mainWindow.isDestroyed()) return;
    mainWindow.webContents.send("kepler:clipboard-history:open-shell");
  }

  function showFocusSessionLauncher(): void {
    showLauncher();
    if (!mainWindow || mainWindow.isDestroyed()) return;
    mainWindow.webContents.send("kepler:focus-session:open-shell");
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

  function resolveTrayIconPath(): string | null {
    // Windows tray требует .ico. На macOS `new Tray(path)` не умеет такой файл
    // и бросает исключение, из-за чего startup обрывается раньше регистрации
    // global hotkey. Поэтому platform-specific asset: Windows -> tray.ico,
    // macOS/prod -> icon.png, dev -> dev.png.
    const iconName =
      process.platform === "win32"
        ? isDev
          ? "dev.png"
          : "tray.ico"
        : isDev
          ? "dev.png"
          : "icon.png";
    const candidates: string[] = [];
    if (process.resourcesPath) {
      candidates.push(path.join(process.resourcesPath, iconName));
    }
    candidates.push(path.resolve(dirname, `../build/${iconName}`));
    candidates.push(path.resolve(dirname, `../../build/${iconName}`));
    for (const candidate of candidates) {
      if (existsSync(candidate)) return candidate;
    }
    return null;
  }

  function createTray(): void {
    if (tray) return;
    const iconPath = resolveTrayIconPath();
    try {
      // Путь передаём строкой на Windows: nativeImage.createFromPath не
      // декодирует .ico, а Tray(path) сам выбирает нужный кадр из multi-size ICO.
      // На macOS сюда приходит PNG path.
      tray = new Tray(iconPath ?? nativeImage.createEmpty());
    } catch (e) {
      console.error("[kepler-shell] tray create failed:", e);
      tray = new Tray(nativeImage.createEmpty());
    }
    tray.setToolTip(productName);
    tray.setContextMenu(
      Menu.buildFromTemplate([
        { label: "Открыть", click: () => showLauncher() },
        { label: "Настройки", click: () => openSettings() },
        { type: "separator" },
        {
          label: "Выход",
          click: () => {
            quitApplication();
          },
        },
      ]),
    );
    tray.on("click", () => showLauncher());
  }

  function setTrayVisible(enabled: boolean): void {
    if (enabled) {
      createTray();
      return;
    }
    tray?.destroy();
    tray = null;
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
      if (launcherHidden) showLauncher();
      else hideLauncher();
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
    showClipboardHistoryLauncher,
    showFocusSessionLauncher,
    setLauncherExpanded,
    setTrayVisible,
    registerLauncherHotkeys,
    getMainWindow: () => mainWindow,
    isLauncherHidden: () => launcherHidden,
  };
}
