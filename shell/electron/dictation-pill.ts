// Dictation pill — frameless overlay window поверх активного окна. Phase 1
// (Groq cloud). Pill renderer владеет audio capture (Web Audio API);
// kepler-backend владеет state machine, Groq запросом, и инжектом текста
// через enigo + clipboard restore.
//
// Lifecycle:
//   1. Global hotkey press → toggleDictation()
//   2. Если idle → capture_foreground_window (backend сохраняет HWND ДО
//      того как мы покажем pill) → start_recording → showPill →
//      `kepler:dictation:command` { kind: "start" } к renderer'у.
//      Renderer запускает getUserMedia + AnalyserNode + Int16 PCM buffer.
//   3. Hotkey press снова (toggle mode) → `command` { kind: "stop" }.
//      Renderer кодирует WAV → base64 → ark.request("dictation.submit_audio").
//      Backend транскрибит + инжектит. На success — renderer вызывает
//      `pillFinished()` → main hide'ит окно.
//   4. Esc в pill → renderer вызывает ark.request("dictation.cancel") +
//      `pillFinished()`.
//
// Headless: окно не показывается (KOSMOS_HEADLESS=1 или KOSMOS_TEST_MODE=1),
// но IPC handlers и state живут — чтобы Playwright мог driver'ить state
// machine без видимого окна.

import { BrowserWindow, ipcMain, screen, webContents as electronWebContents } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { awaitArkReady } from "./main";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WIDTH = 240;
const HEIGHT = 72;
const BOTTOM_MARGIN = 100;

let pillWindow: BrowserWindow | null = null;
let pillReady: Promise<void> | null = null;
let isRecording = false;

function isHeadless(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
}

function createPill(): BrowserWindow {
  // Позиционируем над cursor monitor'ом (более интуитивно чем primary
  // если у юзера multi-monitor setup).
  const cursor = screen.getCursorScreenPoint();
  const display = screen.getDisplayNearestPoint(cursor);
  const area = display.workArea;
  const x = area.x + Math.floor((area.width - WIDTH) / 2);
  // Снизу экрана, с отступом BOTTOM_MARGIN. `workArea` уже исключает taskbar.
  const y = area.y + area.height - HEIGHT - BOTTOM_MARGIN;

  const win = new BrowserWindow({
    width: WIDTH,
    height: HEIGHT,
    x,
    y,
    show: false,
    frame: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    transparent: true,
    backgroundColor: "#00000000",
    roundedCorners: true,
    // КРИТИЧНО: focusable: false — pill НЕ ворует фокус с активного окна.
    // Иначе Ctrl+V после inject улетит в pill (а не в Telegram / редактор).
    focusable: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      // Audio capture не должен throttle'иться когда pill теряет focus
      // (а он его никогда и не получает с focusable: false).
      backgroundThrottling: false,
    },
  });

  win.setAlwaysOnTop(true, "screen-saver", 1);

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void win.loadURL(`${devUrl}#dictation-pill`);
  } else {
    void win.loadFile(path.join(__dirname, "..", "dist", "index.html"), {
      hash: "dictation-pill",
    });
  }

  win.on("closed", () => {
    pillWindow = null;
    pillReady = null;
  });

  // Renderer должен иметь shot подписаться на `kepler:dictation:command` ДО
  // того как мы пошлём первую команду, иначе start теряется и виджет висит
  // в idle. Ждём did-finish-load + один tick (Vue mount).
  pillReady = new Promise<void>((resolve) => {
    win.webContents.once("did-finish-load", () => {
      // Один tick после load — Vue компонент успевает выполнить onMounted
      // (там подписка на onCommand).
      setTimeout(resolve, 50);
    });
  });

  return win;
}

function ensureWindow(): BrowserWindow {
  if (!pillWindow || pillWindow.isDestroyed()) {
    pillWindow = createPill();
  }
  return pillWindow;
}

async function sendPillCommand(cmd: { kind: "start" | "stop" | "cancel" }): Promise<void> {
  const win = pillWindow;
  if (!win || win.isDestroyed()) return;
  if (pillReady) {
    try {
      await pillReady;
    } catch {
      /* ignore */
    }
  }
  if (win.isDestroyed()) return;
  console.log(`[dictation-pill] -> renderer: ${cmd.kind}`);
  win.webContents.send("kepler:dictation:command", cmd);
}

function showPill(): void {
  const win = ensureWindow();
  if (isHeadless()) return;
  if (!win.isVisible()) win.showInactive();
}

function hidePill(): void {
  if (pillWindow && !pillWindow.isDestroyed() && pillWindow.isVisible()) {
    pillWindow.hide();
  }
}

async function callBackend(
  operation: string,
  params: Record<string, unknown> = {},
): Promise<unknown> {
  const ark = await awaitArkReady();
  return ark.invokeOperation({ operation, ...params });
}

/** Главный entry-point из hotkey'я и UI "Тест" кнопки. */
export async function toggleDictation(): Promise<void> {
  if (!isRecording) {
    // 1. capture HWND ДО показа pill — иначе foreground станет pill
    //    (даже с focusable:false есть race до showInactive).
    try {
      await callBackend("dictation.capture_foreground_window");
    } catch (e) {
      console.error("[dictation-pill] capture_foreground_window failed:", e);
    }
    // 2. transition Idle → Recording.
    try {
      await callBackend("dictation.start_recording");
    } catch (e) {
      console.error("[dictation-pill] start_recording failed:", e);
      return;
    }
    isRecording = true;
    showPill();
    await sendPillCommand({ kind: "start" });
  } else {
    isRecording = false;
    // Renderer сам отправит submit_audio и далее pillFinished.
    await sendPillCommand({ kind: "stop" });
  }
}

export async function cancelDictation(): Promise<void> {
  if (!isRecording) return;
  isRecording = false;
  await sendPillCommand({ kind: "cancel" });
  hidePill();
  try {
    await callBackend("dictation.cancel");
  } catch (e) {
    console.error("[dictation-pill] backend cancel failed:", e);
  }
}

// ---------------------------------------------------------------------------
// IPC handlers
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:dictation:toggle", async () => {
  await toggleDictation();
  return { ok: true };
});

ipcMain.handle("kepler:dictation:cancel", async () => {
  await cancelDictation();
  return { ok: true };
});

ipcMain.handle("kepler:dictation:pill-finished", () => {
  isRecording = false;
  hidePill();
  return { ok: true };
});

// ---------------------------------------------------------------------------
// Hotkey routing
//
// Все hotkey'и идут через Rust WH_KEYBOARD_LL hook
// (`services/kepler-backend/src/dictation/hotkey_hook.rs`). Hook эмитит:
//   • `dictation_toggle_trigger` для Toggle mode
//   • `dictation_ptt_trigger { phase: "down" | "up" }` для PTT mode
// Hook intercept'ит match'нувшийся accelerator → системные shortcut'ы
// (Win+H = Voice Typing, Win+Space = переключатель раскладок) не сработают.
// Electron `globalShortcut.register` не используем — он опирается на Win32
// `RegisterHotKey`, который не может занять системные shortcut'ы.
// ---------------------------------------------------------------------------

/** Текущий trigger_mode из backend config'а — для diff'а при config_changed. */
let currentTriggerMode: "toggle" | "push_to_talk" = "toggle";

function applyHotkeyForMode(_hotkey: string, mode: "toggle" | "push_to_talk"): void {
  currentTriggerMode = mode;
  // Hook сам перерегистрируется на backend стороне (`apply_ptt_hook` в
  // host.rs вызывается на каждый update_config). Тут ничего не делаем —
  // функция оставлена для future expansion (например smoke-test'а).
}

/** Warmup pill window на старте, чтобы первый toggleDictation не платил
 * за создание BrowserWindow + load bundle (≈ 600-1500ms на холодном Electron).
 * Создаём окно скрытым (`show: false` уже стоит в createPill); первый toggle
 * только showInactive + IPC send. В headless / тестах ничего не делаем. */
function warmupPill(): void {
  if (isHeadless()) return;
  if (pillWindow && !pillWindow.isDestroyed()) return;
  ensureWindow();
}

/** Вызывается из main.ts на старте (после ARK ready) — читает hotkey + mode
    из backend config'а и регистрирует. Также подписывается на
    `dictation_config_changed` для перерегистрации и на `dictation_ptt_trigger`
    для PTT-режима. */
export async function setupDictationHotkey(): Promise<void> {
  try {
    const ark = await awaitArkReady();
    const cfg = (await ark.invokeOperation({ operation: "dictation.get_config" })) as
      | { config?: { hotkey?: string; triggerMode?: "toggle" | "push_to_talk" } }
      | undefined;
    const hotkey = cfg?.config?.hotkey ?? "Ctrl+Shift+;";
    const mode = cfg?.config?.triggerMode ?? "toggle";
    applyHotkeyForMode(hotkey, mode);
    // Idle warmup: отложить создание pill window на 3s после старта shell'а
    // и сделать его только когда event loop свободен. Цель — не платить за
    // транспарентное BrowserWindow + DWM композицию + Vue bundle загрузку
    // во время startup'а (это съедает +200-400ms ready-time). 3s — компромисс
    // между «не успел warmup до первого нажатия» и «не толкаемся за CPU
    // с launcher mount + ark connect + extension scan».
    setTimeout(() => {
      // setImmediate уводит вызов на следующий tick event loop'а, давая
      // приоритет любым ожидающим тяжёлым задачам.
      setImmediate(warmupPill);
    }, 3000);

    ark.onArkEvent((e: { event?: string; phase?: "down" | "up" }) => {
      // 1. Re-apply mode на config_changed event (hook сам перерегистрируется
      // на backend стороне в `apply_ptt_hook`; здесь только локальный state).
      if (e.event === "dictation_config_changed") {
        void (async () => {
          try {
            const updated = (await ark.invokeOperation({
              operation: "dictation.get_config",
            })) as
              | { config?: { hotkey?: string; triggerMode?: "toggle" | "push_to_talk" } }
              | undefined;
            const nextHotkey = updated?.config?.hotkey ?? "Ctrl+Shift+;";
            const nextMode = updated?.config?.triggerMode ?? "toggle";
            if (nextMode !== currentTriggerMode) {
              applyHotkeyForMode(nextHotkey, nextMode);
            }
          } catch (err) {
            console.error("[dictation-pill] re-apply hotkey mode failed:", err);
          }
        })();
        return;
      }
      // 2. Toggle trigger от Rust hook'а (Toggle mode): один event на каждое
      // нажатие, семантика идентична globalShortcut callback'у.
      if (e.event === "dictation_toggle_trigger") {
        void toggleDictation();
        return;
      }
      // 3. PTT trigger от Rust hook'а (PTT mode): эквивалент press/release.
      // Семантика: down → toggleDictation (старт записи); up → toggleDictation
      // (стоп + отправка). Те же два вызова что юзер делал бы вручную.
      if (e.event === "dictation_ptt_trigger") {
        void toggleDictation();
        return;
      }
      // 4. Capture events для Settings → Диктация → Горячая клавиша.
      // Forward'им на все живые webContents (Settings отдельным окном).
      if (e.event === "dictation_capture_key" || e.event === "dictation_capture_cancelled") {
        for (const wc of electronWebContents.getAllWebContents()) {
          if (!wc.isDestroyed()) {
            wc.send("kepler:dictation:capture", e);
          }
        }
      }
    });
  } catch (e) {
    console.error("[dictation-pill] setupDictationHotkey failed:", e);
  }
}
