import { BrowserWindow, ipcMain } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { awaitArkReady } from "./main";
import { applyFocusBlock } from "./focus-block";
import { setFocusState, type FocusState } from "./focus-widget";

type PomodoroPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface PomodoroState {
  phase: PomodoroPhase;
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  phaseEndsAtMs?: number | null;
  title?: string;
  tasks?: Array<{ id: string; title: string }>;
}

interface ArkObjectLike {
  id: string;
  typeId?: string;
  type_id?: string;
  title?: string | null;
  contentJson?: unknown;
  content_json?: unknown;
  propsJson?: Record<string, unknown>;
  props_json?: Record<string, unknown>;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  deletedAt?: string | null;
  deleted_at?: string | null;
}

interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

interface FocusActiveState {
  active: boolean;
  blocklist_id?: string | null;
  started_at?: string | null;
}

interface DelphiTask {
  id: string;
  title: string;
  status?: string | null;
}

interface StartFocusSessionInput {
  title: string;
  durationMin: number;
  taskId?: string | null;
  taskTitle?: string | null;
  mode?: "block" | "allow";
  categoryIds?: string[];
  blocklistId?: string | null;
}

interface FocusSessionSnapshot {
  pomodoro: PomodoroState;
  focus: FocusActiveState;
  runningEntryId: string | null;
}

let currentEntryId: string | null = null;
let lastWorkContext: { title: string; taskId: string | null; taskTitle: string | null } | null =
  null;
let lastCategoryIds: string[] = [];
let backendEventsUnsubscribe: (() => void) | null = null;
let pendingWorkCompletion = false;
let sideEffectQueue: Promise<void> = Promise.resolve();
let shellOpener: (() => void) | null = null;

export function setFocusSessionShellOpener(opener: () => void): void {
  shellOpener = opener;
}

export function openFocusSessionShell(): void {
  if (shellOpener) {
    shellOpener();
    return;
  }
  console.warn("[focus-session] shell opener is not registered");
}

function broadcastFocusSessionUpdated(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send("kepler:focus-session:updated");
    }
  }
}

function toProps(record: ArkObjectLike): Record<string, unknown> {
  return (record.propsJson ?? record.props_json ?? {}) as Record<string, unknown>;
}

function makeEntryId(): string {
  return `te-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

async function invoke<T = unknown>(
  operation: string,
  params?: Record<string, unknown>,
): Promise<T> {
  const client = await awaitArkReady();
  return client.invokeOperation({ operation, ...params } as {
    operation: string;
    [key: string]: unknown;
  }) as Promise<T>;
}

async function listRunningPomodoroEntries(): Promise<ArkObjectLike[]> {
  const list = await invoke<ArkObjectLike[]>("list_running_time_entries", {
    source: "pomodoro",
  });
  return Array.isArray(list) ? list : [];
}

async function rehydrateRunningEntry(): Promise<string | null> {
  const running = await listRunningPomodoroEntries();
  const target = running.find((entry) => {
    const startedAt = toProps(entry).startedAt;
    return typeof startedAt === "string" && startedAt.length > 0;
  });
  currentEntryId = target?.id ?? null;
  if (target) {
    const props = toProps(target);
    lastWorkContext = {
      title: target.title ?? "Фокус",
      taskId: typeof props.taskId === "string" ? props.taskId : null,
      taskTitle: typeof props.taskTitle === "string" ? props.taskTitle : null,
    };
  }
  return currentEntryId;
}

async function startTimeEntry(ctx: {
  title: string;
  taskId: string | null;
  taskTitle: string | null;
}): Promise<string> {
  const now = new Date().toISOString();
  const id = makeEntryId();
  const record = {
    id,
    typeId: "time_entry_obj",
    title: ctx.title,
    contentJson: {},
    propsJson: {
      startedAt: now,
      endedAt: null,
      source: "pomodoro",
      billable: false,
      taskId: ctx.taskId,
      taskTitle: ctx.taskTitle,
      completed: false,
    },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };
  await invoke("upsert_object", { object: record });
  currentEntryId = id;
  lastWorkContext = ctx;
  return id;
}

async function closeTimeEntry(completed: boolean): Promise<void> {
  const id = currentEntryId ?? (await rehydrateRunningEntry());
  if (!id) return;
  const existing = await invoke<ArkObjectLike | null>("get_object", { id });
  if (!existing) {
    currentEntryId = null;
    return;
  }
  const now = new Date().toISOString();
  const ctx = lastWorkContext;
  const props = {
    ...toProps(existing),
    endedAt: now,
    completed,
    taskId: ctx?.taskId ?? toProps(existing).taskId ?? null,
    taskTitle: ctx?.taskTitle ?? toProps(existing).taskTitle ?? null,
  };
  const record = {
    id: existing.id,
    typeId: existing.typeId ?? existing.type_id ?? "time_entry_obj",
    title: ctx?.title ?? existing.title ?? "Фокус",
    contentJson: existing.contentJson ?? existing.content_json ?? {},
    propsJson: props,
    createdAt: existing.createdAt ?? existing.created_at ?? now,
    updatedAt: now,
    deletedAt: existing.deletedAt ?? existing.deleted_at ?? null,
  };
  await invoke("upsert_object", { object: record });
  currentEntryId = null;
}

function enqueueSideEffect(work: () => Promise<void>): Promise<void> {
  const next = sideEffectQueue.then(work, work).catch((e) => {
    console.error("[focus-session] side-effect failed:", e);
  });
  sideEffectQueue = next;
  return next;
}

async function applyFocusState(active: boolean, categoryIds?: string[]): Promise<void> {
  const ids = categoryIds?.filter(Boolean) ?? [];
  if (active && ids.length > 0) {
    lastCategoryIds = ids;
    await invoke("focus.set_active_state", { active: true, blocklist_id: ids[0] });
    const domains = await resolveCategoryDomains(ids);
    await applyFocusBlock({ active: true, domains });
    setFocusState({ blockingActive: true });
  } else {
    await invoke("focus.set_active_state", { active: false });
    await applyFocusBlock({ active: false, domains: [] });
    setFocusState({ blockingActive: false });
  }
}

async function resolveBlocklistDomains(blocklistId: string): Promise<string[]> {
  try {
    const resolved = await invoke<{ domains?: string[] }>("focus.resolve_blocklist_domains", {
      id: blocklistId,
    });
    return Array.isArray(resolved?.domains) ? resolved.domains : [];
  } catch {
    const resp = await invoke<{ blocklists?: FocusBlocklist[] }>("focus.list_blocklists");
    const found = resp.blocklists?.find((b) => b.id === blocklistId);
    return found?.domains ?? [];
  }
}

async function resolveCategoryDomains(categoryIds: string[]): Promise<string[]> {
  const merged = new Set<string>();
  for (const id of categoryIds) {
    const domains = await resolveBlocklistDomains(id);
    for (const domain of domains) merged.add(domain);
  }
  return Array.from(merged).sort((a, b) => a.localeCompare(b));
}

function deriveFocusWidgetPatch(raw: PomodoroState): Partial<FocusState> {
  const phase = raw.phase ?? "idle";
  const isPaused = raw.isPaused === true;
  const activeTimer = raw.isRunning === true && !isPaused && phase !== "idle";
  const widgetActive = phase !== "idle";
  const mode: FocusState["mode"] = phase === "work" ? "work" : "break";
  const title = (raw.title ?? "").trim();
  const firstTask = raw.tasks?.[0];
  return {
    active: widgetActive,
    remainingSec: Math.max(0, Math.ceil((raw.remainingMs ?? 0) / 1000)),
    totalSec: Math.max(0, Math.ceil((raw.totalMs ?? 0) / 1000)),
    label: title || firstTask?.title || (mode === "work" ? "Фокус" : "Перерыв"),
    mode,
    isPaused,
    phaseEndsAtMs: activeTimer ? (raw.phaseEndsAtMs ?? null) : null,
  };
}

async function snapshot(): Promise<FocusSessionSnapshot> {
  const [pomodoro, focus] = await Promise.all([
    invoke<PomodoroState>("pomodoro.get_state"),
    invoke<FocusActiveState>("focus.get_active_state").catch(() => ({ active: false })),
  ]);
  if (pomodoro.phase !== "idle" && !currentEntryId) {
    await rehydrateRunningEntry().catch(() => null);
  }
  return {
    pomodoro,
    focus,
    runningEntryId: currentEntryId,
  };
}

export async function getFocusSessionSnapshot(): Promise<FocusSessionSnapshot> {
  return snapshot();
}

async function listTasks(): Promise<DelphiTask[]> {
  const list = await invoke<ArkObjectLike[]>("list_objects_by_type", { type_id: "task_obj" });
  return (Array.isArray(list) ? list : [])
    .filter((o) => !o.deletedAt && !o.deleted_at)
    .map((o) => {
      const props = toProps(o);
      const status = typeof props.status === "string" ? props.status : null;
      const isCompleted = props.isCompleted === true || props.is_completed === true;
      const isCancelled = props.isCancelled === true || props.is_cancelled === true;
      const isTrashed = props.isTrashed === true || props.is_trashed === true;
      return {
        id: o.id,
        title: o.title ?? "",
        status,
        isCompleted,
        isCancelled,
        isTrashed,
      };
    })
    .filter((t) => {
      if (t.title.length === 0) return false;
      if (t.isCompleted || t.isCancelled || t.isTrashed) return false;
      return t.status !== "done" && t.status !== "canceled" && t.status !== "cancelled";
    })
    .map(({ id, title, status }) => ({ id, title, status }));
}

async function listBlocklists(): Promise<FocusBlocklist[]> {
  const resp = await invoke<{ blocklists?: FocusBlocklist[] }>("focus.list_blocklists");
  return resp.blocklists ?? [];
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
  if (input.mode === "allow") {
    throw new Error("Режим Allow требует app/browser-level блокировки и пока недоступен");
  }
  lastCategoryIds = categoryIds;
  if (currentEntryId) {
    await enqueueSideEffect(() => closeTimeEntry(false));
  } else {
    await rehydrateRunningEntry().catch(() => null);
    if (currentEntryId) await enqueueSideEffect(() => closeTimeEntry(false));
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
  await applyFocusState(categoryIds.length > 0, categoryIds);
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

async function pauseFocusSession(): Promise<FocusSessionSnapshot> {
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
  if (state.phase === "work" && state.isRunning && !state.isPaused) {
    await startTimeEntry({
      title: state.title || lastWorkContext?.title || "Фокус",
      taskId: state.tasks?.[0]?.id ?? lastWorkContext?.taskId ?? null,
      taskTitle: state.tasks?.[0]?.title ?? lastWorkContext?.taskTitle ?? null,
    });
  }
  setFocusState(deriveFocusWidgetPatch(state));
  if (state.phase === "work" && state.isRunning && !state.isPaused && lastCategoryIds.length > 0) {
    await applyFocusState(true, lastCategoryIds);
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
  const state = await invoke<PomodoroState>("pomodoro.stop");
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  await applyFocusState(false);
  lastCategoryIds = [];
  const next = await snapshot();
  broadcastFocusSessionUpdated();
  return next;
}

export async function stopFocusSessionCommand(): Promise<FocusSessionSnapshot> {
  return stopFocusSession();
}

export async function toggleFocusSessionCommand(): Promise<void> {
  const state = await snapshot();
  if (state.pomodoro.phase === "idle") {
    openFocusSessionShell();
    return;
  }
  await stopFocusSession();
}

ipcMain.handle("kepler:focus-session:open", () => {
  openFocusSessionShell();
});
ipcMain.handle("kepler:focus-session:snapshot", () => snapshot());
ipcMain.handle("kepler:focus-session:list-tasks", () => listTasks());
ipcMain.handle("kepler:focus-session:list-blocklists", () => listBlocklists());
ipcMain.handle("kepler:focus-session:start", (_e, input: StartFocusSessionInput) =>
  startFocusSession(input),
);
ipcMain.handle("kepler:focus-session:pause", () => pauseFocusSession());
ipcMain.handle("kepler:focus-session:resume", () => resumeFocusSession());
ipcMain.handle("kepler:focus-session:skip", () => skipFocusSession());
ipcMain.handle("kepler:focus-session:stop", () => stopFocusSession());

export function setupFocusSessionBackendSync(opts: { arkClient: ArkClient }): void {
  teardownFocusSessionBackendSync();
  backendEventsUnsubscribe = opts.arkClient.onArkEvent((event) => {
    if (event.event === "pomodoro_finished" && event.finished === "work") {
      pendingWorkCompletion = true;
      return;
    }
    if (event.event !== "pomodoro_phase_changed") return;
    const from = (event as { from?: PomodoroPhase }).from ?? "idle";
    const to = (event as { to?: PomodoroPhase }).to ?? "idle";
    if (from === "work") {
      const completed = pendingWorkCompletion;
      pendingWorkCompletion = false;
      void enqueueSideEffect(async () => {
        await closeTimeEntry(completed);
        if (to !== "work") {
          await applyFocusState(false);
          lastCategoryIds = [];
        }
        broadcastFocusSessionUpdated();
      });
    }
  });
}

export function teardownFocusSessionBackendSync(): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-session] teardown unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
  pendingWorkCompletion = false;
}
