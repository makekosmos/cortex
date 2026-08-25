import { BrowserWindow } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { applyFocusBlock } from "./focus-block";
import { showFocusCompletionOverlay } from "./focus-overlay";
import { setFocusState, setFocusWidgetSessionActions } from "./focus-widget";
import {
  setupFocusSessionBackendSync as setupFocusSessionBackendSyncSubscription,
  teardownFocusSessionBackendSync as teardownFocusSessionBackendSyncSubscription,
} from "./focus-session-backend-sync";
import { createFocusSessionDataApi } from "./focus-session-data";
import {
  blockedAppsOrEmpty,
  compactStrings,
  deriveFocusWidgetPatch,
  normalizeBlockedAppsOrEmpty,
} from "./focus-session-helpers";
import {
  killRunningBlockedApps,
  snoozeBlockedFocusApp,
  startProcessWatcher,
  stopProcessWatcher,
} from "./focus-session-process-watcher";
import { registerFocusSessionIpc } from "./focus-session-ipc";
import {
  createFocusTimeEntryApi,
  getCurrentEntryId,
  getLastWorkContext,
} from "./focus-session-time-entry";
import {
  getBlockedAppNotifier,
  openFocusSessionShell,
  requireFocusSessionRuntime,
} from "./focus-session-runtime";
import type {
  FocusActiveState,
  FocusBlockedApp,
  FocusSessionSnapshot,
  PomodoroState,
  StartFocusSessionInput,
} from "./focus-session-types";

export {
  openFocusSessionShell,
  setBlockedAppNotifier,
  setFocusSessionRuntime,
  setFocusSessionShellOpener,
} from "./focus-session-runtime";

let lastCategoryIds: string[] = [];
let lastBlockedAppIds: string[] = [];
let lastBlockedApps: FocusBlockedApp[] = [];
let lastRawBlockedApps: FocusBlockedApp[] = [];
let sideEffectQueue: Promise<void> = Promise.resolve();
type ArkRequest = Parameters<ArkClient["invokeOperation"]>[0];
type ArkRequestParams = Omit<ArkRequest, "operation">;
type ArkValue = ArkRequestParams[string];

function isArkValue<T>(value: T): value is T & ArkValue {
  if (value === undefined || value === null) return true;
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return true;
  }
  if (Array.isArray(value)) return value.every(isArkValue);
  if (typeof value !== "object") return false;
  return Object.values(value).every(isArkValue);
}

function arkParams(params: Record<string, ArkValue>): ArkRequestParams {
  const out: ArkRequestParams = {};
  for (const [key, value] of Object.entries(params)) {
    if (!isArkValue(value)) throw new Error(`Unsupported ARK parameter: ${key}`);
    out[key] = value;
  }
  return out;
}

function broadcastFocusSessionUpdated(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send("kepler:focus-session:updated");
    }
  }
}

async function invoke<T = unknown>(
  operation: string,
  params?: Record<string, ArkValue>,
): Promise<T> {
  const client = await requireFocusSessionRuntime().awaitArkReady();
  const request: ArkRequest = params ? { operation, ...arkParams(params) } : { operation };
  return client.invokeOperation<T>(request);
}

const { rehydrateRunningEntry, startTimeEntry, closeTimeEntry, markTaskDone } =
  // SAFETY: invoke validates every dynamic ARK value before forwarding it to the typed helper.
  createFocusTimeEntryApi(invoke as Parameters<typeof createFocusTimeEntryApi>[0]);
const { resolveCategoryDomains, listTasks, listBlocklists } = createFocusSessionDataApi(
  // SAFETY: invoke validates every dynamic ARK value before forwarding it to ArkClient.
  invoke as Parameters<typeof createFocusSessionDataApi>[0],
);

function enqueueSideEffect(work: () => Promise<void>): Promise<void> {
  const next = sideEffectQueue.then(work, work).catch((e) => {
    console.error("[focus-session] side-effect failed:", e);
  });
  sideEffectQueue = next;
  return next;
}

function clearBlockingContext(): void {
  stopProcessWatcher();
  lastRawBlockedApps = [];
  lastCategoryIds = [];
  lastBlockedAppIds = [];
  lastBlockedApps = [];
}

async function applyFocusState(
  active: boolean,
  categoryIds?: string[],
  blockedAppIds?: string[],
  blockedApps?: FocusBlockedApp[],
): Promise<void> {
  const ids = compactStrings(categoryIds);
  const appIds = Array.from(new Set(compactStrings(blockedAppIds)));
  const apps = normalizeBlockedAppsOrEmpty(blockedApps);
  if (active && (ids.length > 0 || appIds.length > 0 || apps.length > 0)) {
    lastCategoryIds = ids;
    lastBlockedAppIds = appIds;
    lastBlockedApps = apps;
    await invoke("focus.set_active_state", {
      active: true,
      blocklist_id: ids[0] ?? null,
      blocked_app_ids: appIds,
      blocked_apps: apps,
    });
    const domains = await resolveCategoryDomains(ids);
    await applyFocusBlock({ active: true, domains });
    setFocusState({ blockingActive: true });
  } else {
    await invoke("focus.set_active_state", { active: false });
    await applyFocusBlock({ active: false, domains: [] });
    setFocusState({ blockingActive: false });
  }
}

async function snapshot(): Promise<FocusSessionSnapshot> {
  const [pomodoro, focus] = await Promise.all([
    invoke<PomodoroState>("pomodoro.get_state"),
    invoke<FocusActiveState>("focus.get_active_state").catch(() => ({ active: false })),
  ]);
  if (pomodoro.phase !== "idle" && !getCurrentEntryId()) {
    await rehydrateRunningEntry().catch(() => null);
  }
  return {
    pomodoro,
    focus,
    runningEntryId: getCurrentEntryId(),
  };
}

async function startFocusSession(input: StartFocusSessionInput): Promise<FocusSessionSnapshot> {
  const title = input.title.trim() || input.taskTitle?.trim() || "Фокус";
  const durationMin = Math.max(1, Math.min(24 * 60, Math.round(input.durationMin)));
  const categoryIds =
    input.categoryIds && input.categoryIds.length > 0
      ? input.categoryIds
      : input.blocklistId
        ? [input.blocklistId]
        : [];
  const blockedAppIds = Array.from(new Set(compactStrings(input.blockedAppIds)));
  const rawBlockedApps = blockedAppsOrEmpty(input.blockedApps);
  lastRawBlockedApps = rawBlockedApps;
  const blockedApps = normalizeBlockedAppsOrEmpty(rawBlockedApps);
  if (input.mode === "allow") {
    throw new Error("Режим Allow требует app/browser-level блокировки и пока недоступен");
  }
  lastCategoryIds = categoryIds;
  lastBlockedAppIds = blockedAppIds;
  lastBlockedApps = blockedApps;
  if (getCurrentEntryId()) {
    await enqueueSideEffect(() => closeTimeEntry(false));
  } else {
    await rehydrateRunningEntry().catch(() => null);
    if (getCurrentEntryId()) await enqueueSideEffect(() => closeTimeEntry(false));
  }

  const task =
    input.taskId && input.taskTitle ? { id: input.taskId, title: input.taskTitle } : null;
  const config = {
    workMin: durationMin,
    shortBreakMin: 5,
    longBreakMin: 15,
    pomodorosUntilLongBreak: 4,
    autoStartWork: false,
    autoStartBreak: false,
    title,
    tasks: task ? [task] : [],
    workMinOverride: durationMin,
  };

  const state = await invoke<PomodoroState>("pomodoro.start", { config });
  await startTimeEntry({
    title,
    taskId: task?.id ?? null,
    taskTitle: task?.title ?? null,
  });
  setFocusState(deriveFocusWidgetPatch(state));
  await applyFocusState(
    categoryIds.length > 0 || blockedAppIds.length > 0 || blockedApps.length > 0,
    categoryIds,
    blockedAppIds,
    blockedApps,
  );
  if (rawBlockedApps.length > 0) {
    await killRunningBlockedApps(lastRawBlockedApps, getBlockedAppNotifier());
  }
  startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

async function pauseFocusSession(): Promise<FocusSessionSnapshot> {
  stopProcessWatcher();
  const state = await invoke<PomodoroState>("pomodoro.pause");
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  await applyFocusState(false);
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function pauseFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return pauseFocusSession();
}

async function resumeFocusSession(): Promise<FocusSessionSnapshot> {
  const state = await invoke<PomodoroState>("pomodoro.resume");
  const lastWorkContext = getLastWorkContext();
  if (state.phase === "work" && state.isRunning && !state.isPaused) {
    await startTimeEntry({
      title: state.title || lastWorkContext?.title || "Фокус",
      taskId: state.tasks?.[0]?.id ?? lastWorkContext?.taskId ?? null,
      taskTitle: state.tasks?.[0]?.title ?? lastWorkContext?.taskTitle ?? null,
    });
  }
  setFocusState(deriveFocusWidgetPatch(state));
  if (
    state.phase === "work" &&
    state.isRunning &&
    !state.isPaused &&
    (lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0)
  ) {
    await applyFocusState(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps);
    startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
  }
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function resumeFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return resumeFocusSession();
}

async function skipFocusSession(): Promise<FocusSessionSnapshot> {
  const state = await invoke<PomodoroState>("pomodoro.skip");
  setFocusState(deriveFocusWidgetPatch(state));
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function skipFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return skipFocusSession();
}

async function stopFocusSession(): Promise<FocusSessionSnapshot> {
  clearBlockingContext();
  const state = await invoke<PomodoroState>("pomodoro.stop");
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  await applyFocusState(false);
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function stopFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return stopFocusSession();
}

// «Выполнена»: останавливает сессию (time_entry completed=true) и помечает
// привязанную задачу выполненной. Без задачи — эквивалент stop.
async function completeFocusSession(): Promise<FocusSessionSnapshot> {
  const current = await snapshot();
  const task = current.pomodoro.tasks?.[0] ?? null;
  const taskId = task?.id ?? null;
  clearBlockingContext();
  const state = await invoke<PomodoroState>("pomodoro.stop");
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(true));
  if (taskId) {
    await markTaskDone(taskId);
  }
  await applyFocusState(false);
  showFocusCompletionOverlay(
    task?.title?.trim() || current.pomodoro.title?.trim() || "Фокус завершён",
  );
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function toggleFocusSessionCommand(): Promise<void> {
  const state = await snapshot();
  if (state.pomodoro.phase === "idle") {
    openFocusSessionShell();
    return;
  }
  await stopFocusSession();
}

setFocusWidgetSessionActions({
  pause: pauseFocusSession,
  resume: resumeFocusSession,
  skip: skipFocusSession,
  stop: stopFocusSession,
  complete: completeFocusSession,
});

registerFocusSessionIpc({
  open: openFocusSessionShell,
  snapshot,
  listTasks,
  listBlocklists,
  start: startFocusSession,
  pause: pauseFocusSession,
  resume: resumeFocusSession,
  skip: skipFocusSession,
  stop: stopFocusSession,
  complete: completeFocusSession,
  snoozeApp: (appId) => snoozeBlockedFocusApp(appId, lastRawBlockedApps),
});

export function setupFocusSessionBackendSync(opts: { arkClient: ArkClient }): void {
  setupFocusSessionBackendSyncSubscription({
    arkClient: opts.arkClient,
    closeTimeEntry,
    applyFocusState,
    enqueueSideEffect,
    onWorkEnded: clearBlockingContext,
    broadcastUpdated: broadcastFocusSessionUpdated,
  });
}

export function teardownFocusSessionBackendSync(): void {
  teardownFocusSessionBackendSyncSubscription();
  stopProcessWatcher();
}
