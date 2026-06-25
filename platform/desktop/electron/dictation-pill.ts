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

import type { ArkClient } from "@kosmos/ark";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PILL_WIDTH = 380;
const PILL_HEIGHT = 126;
// Window == pill size. Any transparent padding around the pill is composited
// white by DWM on Win32 for tiny transparent always-on-top windows (the same
// reason focus-widget keeps its content filling the whole window). Native
// `roundedCorners` rounds the corners; the renderer root fills edge-to-edge.
const WINDOW_PADDING = 0;
const WIDTH = PILL_WIDTH + WINDOW_PADDING * 2;
const HEIGHT = PILL_HEIGHT + WINDOW_PADDING * 2;
const BOTTOM_MARGIN = 100;

let pillWindow: BrowserWindow | null = null;
let pillReady: Promise<void> | null = null;
let isRecording = false;
/** Reentrancy guard: hook event может прилететь дважды (PTT keydown + keyup
 * в один tick, или дубль подписки если backend reconnect'нул и
 * setupDictationHotkey зашёл повторно). Без этого второй toggle вызывает
 * `start_recording` пока первый ещё в полёте — backend отвечает
 * `state must be idle, got recording`. */
let toggleInFlight = false;
type DictationCommandInvoker = () => Promise<void> | void;
let dictationCommandInvoker: DictationCommandInvoker | null = null;
type DictationRuntime = {
  awaitArkReady: () => Promise<ArkClient>;
  broadcastCommandsUpdated: () => void;
  setDictationHotkeyCache: (hotkey?: string | null) => void;
};
let dictationRuntime: DictationRuntime | null = null;

export function setDictationCommandInvoker(invoker: DictationCommandInvoker | null): void {
  dictationCommandInvoker = invoker;
}

export function setDictationRuntime(runtime: DictationRuntime | null): void {
  dictationRuntime = runtime;
}

function requireDictationRuntime(): DictationRuntime {
  if (!dictationRuntime) {
    throw new Error("dictation runtime bridge is not initialized");
  }
  return dictationRuntime;
}

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
    // ВАЖНО: НЕ задаём backgroundMaterial и НЕ зовём setBackgroundMaterial.
    // На Win11 это включает DWM systembackdrop, который заливает прозрачные
    // пиксели окна белым — а у pill видны прозрачные скруглённые углы
    // (border-radius в CSS), и они светились белым. У focus-widget material
    // тоже есть, но его контент заполняет окно целиком, так что прозрачных
    // зон не видно. Скругление окна делает CSS (DWM roundedCorners на
    // transparent frameless-окне всё равно не работает), поэтому опцию не
    // ставим.
    roundedCorners: false,
    // focusable: true — как у focus-widget. На Win32 non-focusable прозрачное
    // окно композитит белую подложку/кайму по краям (это и был последний
    // источник «белых краёв»). Фокус при этом НЕ воруется: окно показывается
    // через showInactive() (см. showPill), foreground-приложение остаётся
    // активным, и Ctrl+V после inject уходит в него, а не в pill.
    focusable: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      // Audio capture не должен throttle'иться: окно показывается через
      // showInactive() и фактический focus не получает, так что без этого
      // флага Chromium бы прибил таймеры/аудио-граф.
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
  win.setIgnoreMouseEvents(false);
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
  const ark = await requireDictationRuntime().awaitArkReady();
  return ark.invokeOperation({ operation, ...params });
}

async function invokeDictationCommand(): Promise<void> {
  if (dictationCommandInvoker) {
    await dictationCommandInvoker();
    return;
  }
  await toggleDictation();
}

/** Главный entry-point из hotkey'я и UI "Тест" кнопки. */
export async function toggleDictation(): Promise<void> {
  if (toggleInFlight) {
    console.warn("[dictation-pill] toggleDictation re-entry ignored");
    return;
  }
  toggleInFlight = true;
  try {
    if (!isRecording) {
      // 1. capture HWND ДО показа pill — иначе foreground станет pill
      //    (даже с focusable:false есть race до showInactive).
      try {
        await callBackend("dictation.capture_foreground_window");
      } catch (e) {
        console.error("[dictation-pill] capture_foreground_window failed:", e);
      }
      // 2. transition Idle → Recording. Если backend застрял в Recording
      // (например renderer не дошёл до submit_audio из-за пустого pcm или
      // upstream race) — `cancel` сбрасывает state в Idle из любой phase,
      // и мы retry'им start_recording. После cancel state точно Idle.
      let started = false;
      try {
        await callBackend("dictation.start_recording");
        started = true;
      } catch (e) {
        const msg = String(e);
        if (msg.includes("state must be idle")) {
          console.warn("[dictation-pill] backend stuck — calling cancel + retry");
          try {
            await callBackend("dictation.cancel");
          } catch (e2) {
            console.error("[dictation-pill] auto-cancel failed:", e2);
          }
          try {
            await callBackend("dictation.start_recording");
            started = true;
          } catch (e2) {
            console.error("[dictation-pill] start_recording retry failed:", e2);
          }
        } else {
          console.error("[dictation-pill] start_recording failed:", e);
        }
      }
      if (!started) return;
      isRecording = true;
      showPill();
      await sendPillCommand({ kind: "start" });
    } else {
      isRecording = false;
      // Renderer сам отправит submit_audio и далее pillFinished.
      await sendPillCommand({ kind: "stop" });
    }
  } finally {
    toggleInFlight = false;
  }
}

async function cancelDictation(): Promise<void> {
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

ipcMain.handle("kepler:dictation:pill-finished", async () => {
  isRecording = false;
  hidePill();
  // Защитный cancel: если renderer завершил pill (например пустой pcm,
  // permission denied, тех. ошибка) ДО того как backend дошёл до
  // submit_audio → backend застрянет в Recording. Cancel сбрасывает state
  // в Idle no-op'но если уже Idle.
  try {
    await callBackend("dictation.cancel");
  } catch {
    /* ignore — best effort */
  }
  return { ok: true };
});

// ---------------------------------------------------------------------------
// Hotkey routing
//
// Все hotkey'и идут через Rust WH_KEYBOARD_LL hook
// (`platform/runtime/src/dictation/hotkey_hook.rs`). Hook эмитит:
//   • `dictation_toggle_trigger` для Toggle mode
//   • `dictation_ptt_trigger { phase: "down" | "up" }` для PTT mode
// Hook intercept'ит match'нувшийся accelerator → системные shortcut'ы
// (Win+H = Voice Typing, Win+Space = переключатель раскладок) не сработают.
// Electron `globalShortcut.register` не используем — он опирается на Win32
// `RegisterHotKey`, который не может занять системные shortcut'ы.
// ---------------------------------------------------------------------------

/** Unsubscribe от прошлой ARK подписки. setupDictationHotkey может быть
 * вызван повторно (например при reconnect ARK client'а) — без cleanup
 * подписка дублируется и каждый hook event обрабатывается несколько раз,
 * что приводит к `state must be idle, got recording`. */
let arkUnsubscribe: (() => void) | null = null;

function applyHotkeyForMode(_hotkey: string, _mode: "toggle" | "push_to_talk"): void {
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
    const runtime = requireDictationRuntime();
    const ark = await runtime.awaitArkReady();
    const cfg = (await ark.invokeOperation({ operation: "dictation.get_config" })) as
      | { config?: { hotkey?: string; triggerMode?: "toggle" | "push_to_talk" } }
      | undefined;
    const hotkey = cfg?.config?.hotkey ?? "Ctrl+Shift+;";
    const mode = cfg?.config?.triggerMode ?? "toggle";
    runtime.setDictationHotkeyCache(hotkey);
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

    // Cleanup предыдущую подписку — защита от дубля если setupDictationHotkey
    // зашёл повторно (reconnect / hot-reload).
    if (arkUnsubscribe) {
      arkUnsubscribe();
      arkUnsubscribe = null;
    }
    arkUnsubscribe = ark.onArkEvent((e: { event?: string; phase?: "down" | "up" }) => {
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
            runtime.setDictationHotkeyCache(nextHotkey);
            applyHotkeyForMode(nextHotkey, nextMode);
            runtime.broadcastCommandsUpdated();
          } catch (err) {
            console.error("[dictation-pill] re-apply hotkey mode failed:", err);
          }
        })();
        return;
      }
      // 2. Toggle trigger от Rust hook'а (Toggle mode): один event на каждое
      // нажатие, семантика идентична globalShortcut callback'у.
      if (e.event === "dictation_toggle_trigger") {
        console.log("[dictation-pill] hook event: toggle");
        void invokeDictationCommand();
        return;
      }
      // 3. PTT trigger от Rust hook'а (PTT mode): эквивалент press/release.
      // Семантика: down → toggleDictation (старт записи); up → toggleDictation
      // (стоп + отправка). На одно нажатие приходят ДВА event'а — это by
      // design hold-to-record. Если для юзера это выглядит как «запись
      // началась и сразу прекратилась», значит он перепутал PTT с Toggle —
      // поменять в Settings → Диктация → Режим триггера.
      if (e.event === "dictation_ptt_trigger") {
        console.log(`[dictation-pill] hook event: ptt ${e.phase}`);
        void invokeDictationCommand();
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
