// Focus widget — Spotify-mini-player-style плавающий always-on-top окно,
// показывается во время активной pomodoro сессии.

import { app, BrowserWindow } from "electron";
import type { ArkClient } from "@kosmos/ark";
import {
  DEFAULT_FOCUS_WIDGET_STATE,
  deriveFocusStateFromBackend,
  isDefaultFocusLabel,
  type FocusState,
  type PomodoroEventState,
} from "./focus-widget-state";
import {
  createFocusWidgetWindow,
  resetFocusWidgetPosition,
  scheduleFocusWidgetBoundsPersist,
} from "./focus-widget-window";
import { registerFocusWidgetIpcHandlers, type FocusWidgetSessionActions } from "./focus-widget-ipc";

type FocusWidgetRuntime = {
  awaitArkReady: () => Promise<ArkClient>;
};

let widgetWindow: BrowserWindow | null = null;
let currentState: FocusState = { ...DEFAULT_FOCUS_WIDGET_STATE };
let tickTimer: ReturnType<typeof setInterval> | null = null;
let focusSessionOpener: (() => void) | null = null;
let focusWidgetRuntime: FocusWidgetRuntime | null = null;
let focusSessionActions: FocusWidgetSessionActions | null = null;
let backendEventsUnsubscribe: (() => void) | null = null;

export type { FocusState } from "./focus-widget-state";

export function setFocusWidgetFocusSessionOpener(opener: () => void): void {
  focusSessionOpener = opener;
}

export function setFocusWidgetRuntime(runtime: FocusWidgetRuntime | null): void {
  focusWidgetRuntime = runtime;
}

export function setFocusWidgetSessionActions(actions: FocusWidgetSessionActions): void {
  focusSessionActions = actions;
}

function requireFocusWidgetSessionActions(): FocusWidgetSessionActions {
  if (!focusSessionActions) {
    throw new Error("focus widget session actions are not initialized");
  }
  return focusSessionActions;
}

function requireFocusWidgetRuntime(): FocusWidgetRuntime {
  if (!focusWidgetRuntime) {
    throw new Error("focus widget runtime bridge is not initialized");
  }
  return focusWidgetRuntime;
}

function openFocusSessionFromWidget(): void {
  if (!focusSessionOpener) {
    console.warn("[focus-widget] focus session opener is not registered");
    return;
  }
  focusSessionOpener();
}

function ensureWindow(): BrowserWindow {
  if (!widgetWindow || widgetWindow.isDestroyed()) {
    widgetWindow = createFocusWidgetWindow({
      onMove: () => scheduleFocusWidgetBoundsPersist(() => widgetWindow),
      onClosed: () => {
        widgetWindow = null;
      },
    });
  }
  return widgetWindow;
}

function showWidget(): void {
  const win = ensureWindow();
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
    return;
  }
  if (!win.isVisible()) win.showInactive();
}

function hideWidget(): void {
  if (widgetWindow && !widgetWindow.isDestroyed() && widgetWindow.isVisible()) {
    widgetWindow.hide();
  }
}

function broadcastState(): void {
  if (widgetWindow && !widgetWindow.isDestroyed()) {
    try {
      widgetWindow.webContents.send("kepler:focus-widget:state", currentState);
    } catch {
      /* ignore */
    }
  }
}

function recomputeRemainingFromAnchor(): boolean {
  if (currentState.phaseEndsAtMs == null) return false;
  const next = Math.max(0, Math.ceil((currentState.phaseEndsAtMs - Date.now()) / 1000));
  if (currentState.remainingSec === next) return false;
  currentState.remainingSec = next;
  return true;
}

function ensureTickTimer(): void {
  if (tickTimer != null) return;
  tickTimer = setInterval(() => {
    if (!currentState.active || currentState.phaseEndsAtMs == null) {
      clearTickTimer();
      return;
    }
    if (recomputeRemainingFromAnchor()) {
      broadcastState();
    }
  }, 1000);
}

function clearTickTimer(): void {
  if (tickTimer != null) {
    clearInterval(tickTimer);
    tickTimer = null;
  }
}

export function setFocusState(next: Partial<FocusState>): void {
  const previousLabel = currentState.label;
  const shouldKeepCurrentLabel =
    currentState.active &&
    next.active !== false &&
    next.mode === currentState.mode &&
    isDefaultFocusLabel(next.label) &&
    !isDefaultFocusLabel(currentState.label);

  currentState = { ...currentState, ...next };
  if (shouldKeepCurrentLabel) {
    currentState.label = previousLabel;
  }
  if (currentState.active) {
    showWidget();
  } else {
    hideWidget();
  }
  if (currentState.active && currentState.phaseEndsAtMs != null) {
    recomputeRemainingFromAnchor();
    ensureTickTimer();
  } else {
    clearTickTimer();
  }
  broadcastState();
}

function getFocusState(): FocusState {
  return { ...currentState };
}

registerFocusWidgetIpcHandlers({
  getWidgetWindow: () => widgetWindow,
  hideWidget,
  openFocusSessionFromWidget,
  resetWidgetPosition: () => {
    const win = ensureWindow();
    resetFocusWidgetPosition(win);
  },
  setFocusState,
  getFocusState,
  getSessionActions: requireFocusWidgetSessionActions,
  requireRuntime: requireFocusWidgetRuntime,
});

export function setupFocusWidgetBackendSync(opts: { arkClient: ArkClient }): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-widget] previous unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
  backendEventsUnsubscribe = opts.arkClient.onArkEvent((e) => {
    if (
      e.event !== "pomodoro_tick" &&
      e.event !== "pomodoro_phase_changed" &&
      e.event !== "pomodoro_finished"
    ) {
      return;
    }
    const raw = e as unknown as PomodoroEventState;
    const patch = deriveFocusStateFromBackend(raw);
    setFocusState(patch);
  });

  void opts.arkClient
    .invokeOperation({ operation: "pomodoro.get_state" } as { operation: string })
    .then((s) => {
      if (s && typeof s === "object") {
        setFocusState(deriveFocusStateFromBackend(s as PomodoroEventState));
      }
    })
    .catch((err) => {
      console.warn("[focus-widget] initial pomodoro.get_state failed:", err);
    });

  console.log("[focus-widget] subscribed to backend pomodoro events");
}

export function teardownFocusWidgetBackendSync(): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-widget] teardown unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
}

app.on("before-quit", () => {
  clearTickTimer();
  teardownFocusWidgetBackendSync();
  if (widgetWindow && !widgetWindow.isDestroyed()) {
    widgetWindow.destroy();
    widgetWindow = null;
  }
});
