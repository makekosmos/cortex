import { BrowserWindow } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { JsonRecord, JsonValue } from "./extension-permissions";
import {
  applyAuthoritativeFocusState,
  reconcileFocusState as reconcileNativeFocusState,
  resetNativeFocusState,
} from "./focus-enforcement";
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
let focusOperationQueue: Promise<void> = Promise.resolve();
let focusEnforcementError: string | null = null;
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

function focusAppsAsJson(apps: FocusBlockedApp[]): JsonValue[] {
  return apps.map((app) => {
    const value: JsonRecord = { id: app.id, name: app.name, icon: app.icon ?? null };
    if (app.exec_path) value.exec_path = app.exec_path;
    return value;
  });
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

function enqueueFocusOperation<T>(work: () => Promise<T>): Promise<T> {
  const next = focusOperationQueue.then(work, work);
  focusOperationQueue = next.then(
    () => undefined,
    () => undefined,
  );
  return next;
}

function clearBlockingContext(): void {
  stopProcessWatcher();
  lastRawBlockedApps = [];
  lastCategoryIds = [];
  lastBlockedAppIds = [];
  lastBlockedApps = [];
}

async function restoreBlockingContextFromPersisted(focus: FocusActiveState): Promise<void> {
  lastCategoryIds = focus.blocklist_id?.trim() ? [focus.blocklist_id.trim()] : [];
  lastBlockedAppIds = compactStrings(focus.blocked_app_ids);
  const persistedApps = normalizeBlockedAppsOrEmpty(focus.blocked_apps);
  let indexedApps: FocusBlockedApp[] = [];
  if (lastBlockedAppIds.length > 0) {
    indexedApps = await invoke<{ apps?: FocusBlockedApp[] }>("app_index.list_all", { limit: 500 })
      .then((result) => (Array.isArray(result.apps) ? result.apps : []))
      .catch(() => []);
  }
  const appsById = new Map(indexedApps.map((app) => [app.id, app]));
  lastRawBlockedApps = lastBlockedAppIds
    .map((id) => appsById.get(id) ?? persistedApps.find((app) => app.id === id))
    .filter((app): app is FocusBlockedApp => app !== undefined);
  lastBlockedApps = normalizeBlockedAppsOrEmpty([...persistedApps, ...lastRawBlockedApps]);
}

async function applyFocusStateUnsafe(
  active: boolean,
  categoryIds?: string[],
  blockedAppIds?: string[],
  blockedApps?: FocusBlockedApp[],
): Promise<void> {
  if (
    categoryIds === undefined &&
    blockedAppIds === undefined &&
    blockedApps === undefined &&
    lastCategoryIds.length === 0 &&
    lastBlockedAppIds.length === 0 &&
    lastBlockedApps.length === 0
  ) {
    const persisted = await invoke<FocusActiveState>("focus.get_active_state").catch(() => null);
    if (persisted?.active) await restoreBlockingContextFromPersisted(persisted);
  }
  const ids = categoryIds === undefined ? lastCategoryIds : compactStrings(categoryIds);
  const appIds =
    blockedAppIds === undefined
      ? lastBlockedAppIds
      : Array.from(new Set(compactStrings(blockedAppIds)));
  const apps =
    blockedApps === undefined ? lastBlockedApps : normalizeBlockedAppsOrEmpty(blockedApps);
  if (active) {
    lastCategoryIds = ids;
    lastBlockedAppIds = appIds;
    lastBlockedApps = apps;
  }
  try {
    const transition = await applyAuthoritativeFocusState({
      // SAFETY: all ARK operation results are JSON values from the ArkClient transport.
      request: (operation, params) =>
        invoke(operation, params as Record<string, ArkValue>) as Promise<JsonValue>,
      active,
      blocklistId: ids[0] ?? null,
      blockedAppIds: appIds,
      blockedApps: focusAppsAsJson(apps),
      resolveDomains: (blocklistId) => resolveCategoryDomains([blocklistId]),
    });
    focusEnforcementError = null;
    setFocusState({ blockingActive: transition.nativeActive });
  } catch (error) {
    focusEnforcementError = error instanceof Error ? error.message : String(error);
    setFocusState({ blockingActive: !active });
    broadcastFocusSessionUpdated();
    throw error;
  }
}

async function applyFocusState(
  active: boolean,
  categoryIds?: string[],
  blockedAppIds?: string[],
  blockedApps?: FocusBlockedApp[],
): Promise<void> {
  return enqueueFocusOperation(() =>
    applyFocusStateUnsafe(active, categoryIds, blockedAppIds, blockedApps),
  );
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
    focusError: focusEnforcementError,
  };
}

async function startFocusSessionUnsafe(
  input: StartFocusSessionInput,
): Promise<FocusSessionSnapshot> {
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

  let state: PomodoroState;
  try {
    state = await invoke<PomodoroState>("pomodoro.start", { config });
  } catch (error) {
    clearBlockingContext();
    throw error;
  }
  try {
    await startTimeEntry({
      title,
      taskId: task?.id ?? null,
      taskTitle: task?.title ?? null,
    });
    await applyFocusStateUnsafe(
      categoryIds.length > 0 || blockedAppIds.length > 0 || blockedApps.length > 0,
      categoryIds,
      blockedAppIds,
      blockedApps,
    );
    setFocusState(deriveFocusWidgetPatch(state));
    if (rawBlockedApps.length > 0) {
      await killRunningBlockedApps(lastRawBlockedApps, getBlockedAppNotifier());
    }
    startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
  } catch (error) {
    await invoke<PomodoroState>("pomodoro.stop")
      .then((stopped) => setFocusState(deriveFocusWidgetPatch(stopped)))
      .catch(() => undefined);
    await enqueueSideEffect(() => closeTimeEntry(false)).catch(() => undefined);
    clearBlockingContext();
    throw error;
  }
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

async function pauseFocusSessionUnsafe(): Promise<FocusSessionSnapshot> {
  const hadContext =
    lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0;
  await applyFocusStateUnsafe(false);
  let state: PomodoroState;
  try {
    state = await invoke<PomodoroState>("pomodoro.pause");
  } catch (error) {
    if (hadContext) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps).catch(
        () => undefined,
      );
    }
    throw error;
  }
  stopProcessWatcher();
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function pauseFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return pauseFocusSession();
}

async function resumeFocusSessionUnsafe(): Promise<FocusSessionSnapshot> {
  const state = await invoke<PomodoroState>("pomodoro.resume");
  const lastWorkContext = getLastWorkContext();
  const shouldBlock =
    state.phase === "work" &&
    state.isRunning &&
    !state.isPaused &&
    (lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0);
  try {
    if (state.phase === "work" && state.isRunning && !state.isPaused) {
      await startTimeEntry({
        title: state.title || lastWorkContext?.title || "Фокус",
        taskId: state.tasks?.[0]?.id ?? lastWorkContext?.taskId ?? null,
        taskTitle: state.tasks?.[0]?.title ?? lastWorkContext?.taskTitle ?? null,
      });
    }
    if (shouldBlock) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps);
      startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
    }
    setFocusState(deriveFocusWidgetPatch(state));
  } catch (error) {
    await invoke<PomodoroState>("pomodoro.pause").catch(() => undefined);
    await enqueueSideEffect(() => closeTimeEntry(false)).catch(() => undefined);
    setFocusState({ blockingActive: false });
    throw error;
  }
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function resumeFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return resumeFocusSession();
}

async function skipFocusSessionUnsafe(): Promise<FocusSessionSnapshot> {
  const before = await snapshot();
  const hasContext =
    lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0;
  const leavingWork = before.pomodoro.phase === "work";
  const enteringWork = before.pomodoro.phase !== "work" && before.pomodoro.phase !== "idle";
  if (leavingWork) await applyFocusStateUnsafe(false);
  if (enteringWork && hasContext) await applyFocusStateUnsafe(true);

  let state: PomodoroState;
  try {
    state = await invoke<PomodoroState>("pomodoro.skip");
  } catch (error) {
    if (leavingWork && hasContext) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps).catch(
        () => undefined,
      );
    } else if (enteringWork && hasContext) {
      await applyFocusStateUnsafe(false).catch(() => undefined);
    }
    throw error;
  }

  const shouldBlock =
    state.phase === "work" &&
    state.isRunning &&
    !state.isPaused &&
    (lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0);
  if (shouldBlock) {
    if (!enteringWork) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps);
    }
    startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
  } else if (leavingWork || enteringWork) {
    if (leavingWork ? state.phase !== "work" : enteringWork) {
      await applyFocusStateUnsafe(false);
    }
    stopProcessWatcher();
  }
  setFocusState(deriveFocusWidgetPatch(state));
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function skipFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return skipFocusSession();
}

async function stopFocusSessionUnsafe(): Promise<FocusSessionSnapshot> {
  const hadContext =
    lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0;
  await applyFocusStateUnsafe(false);
  let state: PomodoroState;
  try {
    state = await invoke<PomodoroState>("pomodoro.stop");
  } catch (error) {
    if (hadContext) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps).catch(
        () => undefined,
      );
    }
    throw error;
  }
  stopProcessWatcher();
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  clearBlockingContext();
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function stopFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return stopFocusSession();
}

// «Выполнена»: останавливает сессию (time_entry completed=true) и помечает
// привязанную задачу выполненной. Без задачи — эквивалент stop.
async function completeFocusSessionUnsafe(): Promise<FocusSessionSnapshot> {
  const current = await snapshot();
  const task = current.pomodoro.tasks?.[0] ?? null;
  const taskId = task?.id ?? null;
  const hadContext =
    lastCategoryIds.length > 0 || lastBlockedAppIds.length > 0 || lastBlockedApps.length > 0;
  await applyFocusStateUnsafe(false);
  let state: PomodoroState;
  try {
    state = await invoke<PomodoroState>("pomodoro.stop");
  } catch (error) {
    if (hadContext) {
      await applyFocusStateUnsafe(true, lastCategoryIds, lastBlockedAppIds, lastBlockedApps).catch(
        () => undefined,
      );
    }
    throw error;
  }
  stopProcessWatcher();
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(true));
  if (taskId) {
    await markTaskDone(taskId);
  }
  clearBlockingContext();
  showFocusCompletionOverlay(
    task?.title?.trim() || current.pomodoro.title?.trim() || "Фокус завершён",
  );
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

function startFocusSession(input: StartFocusSessionInput): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => startFocusSessionUnsafe(input));
}

function pauseFocusSession(): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => pauseFocusSessionUnsafe());
}

function resumeFocusSession(): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => resumeFocusSessionUnsafe());
}

function skipFocusSession(): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => skipFocusSessionUnsafe());
}

function stopFocusSession(): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => stopFocusSessionUnsafe());
}

function completeFocusSession(): Promise<FocusSessionSnapshot> {
  return enqueueFocusOperation(() => completeFocusSessionUnsafe());
}

export async function toggleFocusSessionCommand(): Promise<void> {
  await enqueueFocusOperation(async () => {
    const state = await snapshot();
    if (state.pomodoro.phase === "idle") {
      openFocusSessionShell();
      return;
    }
    await stopFocusSessionUnsafe();
  });
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
    onWorkEnded: (nextPhase) => {
      stopProcessWatcher();
      if (nextPhase === "idle") clearBlockingContext();
    },
    onWorkStarted: () => {
      startProcessWatcher({ blockedApps: lastRawBlockedApps, notifier: getBlockedAppNotifier() });
    },
    onFocusEnforcementError: (active, error) => {
      console.warn("[focus-session] native focus phase transition failed:", error);
      setFocusState({ blockingActive: !active });
      broadcastFocusSessionUpdated();
    },
    broadcastUpdated: broadcastFocusSessionUpdated,
  });
  void enqueueFocusOperation(async () => {
    try {
      const result = await reconcileNativeFocusState({
        // SAFETY: all ARK operation results are JSON values from the ArkClient transport.
        request: (operation, params) =>
          invoke(operation, params as Record<string, ArkValue>) as Promise<JsonValue>,
        resolveDomains: (blocklistId) => resolveCategoryDomains([blocklistId]),
      });
      if (result.state.active) {
        await restoreBlockingContextFromPersisted({
          active: true,
          blocklist_id: result.state.blocklist_id,
          blocked_app_ids: result.state.blocked_app_ids,
          blocked_apps: [],
        });
        const pomodoro = await invoke<PomodoroState>("pomodoro.get_state");
        if (pomodoro.phase === "work" && pomodoro.isRunning && !pomodoro.isPaused) {
          startProcessWatcher({
            blockedApps: lastRawBlockedApps,
            notifier: getBlockedAppNotifier(),
          });
        }
      }
      setFocusState({ blockingActive: result.nativeActive });
    } catch (error) {
      setFocusState({ blockingActive: false });
      console.warn("[focus-session] native focus reconciliation failed:", error);
    }
  });
}

export async function teardownFocusSessionBackendSync(): Promise<void> {
  teardownFocusSessionBackendSyncSubscription();
  stopProcessWatcher();
  try {
    await resetNativeFocusState();
  } catch (error) {
    console.warn("[focus-session] native focus reset during teardown failed:", error);
  }
}
