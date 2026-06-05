import { BrowserWindow, ipcMain } from "electron";
import path from "node:path";
import os from "node:os";
import { spawn } from "node:child_process";
import { writeFileSync, unlinkSync } from "node:fs";
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
  blocked_app_ids?: string[];
  blocked_apps?: FocusBlockedApp[];
  started_at?: string | null;
}

interface FocusBlockedApp {
  id: string;
  name: string;
  icon?: string | null;
  exec_path?: string | null;
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
  blockedAppIds?: string[];
  blockedApps?: FocusBlockedApp[];
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
let lastBlockedAppIds: string[] = [];
let lastBlockedApps: FocusBlockedApp[] = [];
let lastRawBlockedApps: FocusBlockedApp[] = [];
let watcherProcess: ReturnType<typeof spawn> | null = null;
let backupSweepTimer: ReturnType<typeof setInterval> | null = null;
let backendEventsUnsubscribe: (() => void) | null = null;
let pendingWorkCompletion = false;
let sideEffectQueue: Promise<void> = Promise.resolve();
let shellOpener: (() => void) | null = null;
let blockedAppNotifier:
  | ((app: { id: string; title: string; icon?: string | null }) => void)
  | null = null;
const snoozedBlockedApps = new Map<string, number>();
const lastNotifiedAt = new Map<string, number>();
const SNOOZE_MS = 5 * 60 * 1000;
const NOTIFY_DEBOUNCE_MS = 8000;
const SNOOZE_FILE = path.join(os.tmpdir(), "kosmos-focus-snooze.txt");

export function setFocusSessionShellOpener(opener: () => void): void {
  shellOpener = opener;
}

export function setBlockedAppNotifier(
  notifier: (app: { id: string; title: string; icon?: string | null }) => void,
): void {
  blockedAppNotifier = notifier;
}

export function snoozeBlockedFocusApp(appId: string): void {
  snoozedBlockedApps.set(appId, Date.now() + SNOOZE_MS);
  writeSnoozeFile();
  // Перезаписываем файл по истечении снуза, чтобы PS-watcher снова ловил.
  setTimeout(() => writeSnoozeFile(), SNOOZE_MS + 200);
}

function isBlockedFocusAppSnoozed(appId: string): boolean {
  const until = snoozedBlockedApps.get(appId) ?? 0;
  if (until <= Date.now()) {
    snoozedBlockedApps.delete(appId);
    return false;
  }
  return true;
}

function exeNameOf(app: FocusBlockedApp): string | null {
  if (!app.exec_path) return null;
  const exe = path.basename(app.exec_path).toLowerCase();
  return exe.length >= 4 ? exe : null;
}

// Снуженные exe-имена → файл, который читает PS-watcher на каждом событии.
function writeSnoozeFile(): void {
  const names = new Set<string>();
  for (const app of lastRawBlockedApps) {
    if (isBlockedFocusAppSnoozed(app.id)) {
      const exe = exeNameOf(app);
      if (exe) names.add(exe);
    }
  }
  try {
    writeFileSync(SNOOZE_FILE, Array.from(names).join("\n"), "utf8");
  } catch (e) {
    console.warn("[focus-session] writeSnoozeFile failed:", e);
  }
}

function notifyBlocked(app: FocusBlockedApp): void {
  const now = Date.now();
  const last = lastNotifiedAt.get(app.id) ?? 0;
  if (now - last < NOTIFY_DEBOUNCE_MS) return;
  lastNotifiedAt.set(app.id, now);
  if (blockedAppNotifier) {
    blockedAppNotifier({ id: app.id, title: app.name, icon: app.icon ?? null });
  }
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

async function applyFocusState(
  active: boolean,
  categoryIds?: string[],
  blockedAppIds?: string[],
  blockedApps?: FocusBlockedApp[],
): Promise<void> {
  const ids = categoryIds?.filter(Boolean) ?? [];
  const appIds = Array.from(new Set(blockedAppIds?.filter(Boolean) ?? []));
  const apps = normalizeBlockedApps(blockedApps ?? []);
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

function appByExe(exeName: string): FocusBlockedApp | null {
  const target = exeName.toLowerCase();
  for (const app of lastRawBlockedApps) {
    if (exeNameOf(app) === target) return app;
  }
  return null;
}

// Мгновенный путь: long-lived PowerShell, подписанный на WMI
// __InstanceCreationEvent. WaitForNextEvent() блокируется до старта процесса
// (~100ms), убивает его через Stop-Process до отрисовки окна и пишет имя в
// stdout. Снуз читается из файла на каждом событии. Работает без admin для
// процессов своей сессии.
function startProcessWatcher(): void {
  stopProcessWatcher();
  writeSnoozeFile();

  const exeNames = new Set<string>();
  for (const app of lastRawBlockedApps) {
    const exe = exeNameOf(app);
    if (exe) exeNames.add(exe);
  }
  if (exeNames.size === 0) return;

  const allowedLiteral = Array.from(exeNames)
    .map((n) => `'${n}'`)
    .join(",");
  const snoozePathLiteral = SNOOZE_FILE.replace(/'/g, "''");

  const psScript = [
    `$ErrorActionPreference = 'SilentlyContinue'`,
    `$snoozeFile = '${snoozePathLiteral}'`,
    `$targets = @{}`,
    `@(${allowedLiteral}) | ForEach-Object { $targets[$_] = $true }`,
    `function Get-Snoozed {`,
    `  $s = @{}`,
    `  if (Test-Path $snoozeFile) {`,
    `    foreach ($l in (Get-Content $snoozeFile)) { $t = $l.Trim().ToLower(); if ($t) { $s[$t] = $true } }`,
    `  }`,
    `  return $s`,
    `}`,
    `$query = "SELECT * FROM __InstanceCreationEvent WITHIN 0.1 WHERE TargetInstance ISA 'Win32_Process'"`,
    `$w = New-Object System.Management.ManagementEventWatcher`,
    `$w.Query = New-Object System.Management.WqlEventQuery($query)`,
    `$w.Start()`,
    `while ($true) {`,
    `  try { $evt = $w.WaitForNextEvent() } catch { Start-Sleep -Milliseconds 500; continue }`,
    `  if (-not $evt) { continue }`,
    `  $ti = $evt.TargetInstance`,
    `  $name = ([string]$ti.Name).ToLower()`,
    `  if ($targets.ContainsKey($name)) {`,
    `    $snoozed = Get-Snoozed`,
    `    if (-not $snoozed.ContainsKey($name)) {`,
    `      try { Stop-Process -Id ([int]$ti.ProcessId) -Force } catch {}`,
    `      [Console]::Out.WriteLine('KILLED ' + $name)`,
    `      [Console]::Out.Flush()`,
    `    }`,
    `  }`,
    `}`,
  ].join("\n");

  watcherProcess = spawn(
    "powershell.exe",
    ["-NonInteractive", "-NoProfile", "-Command", psScript],
    { windowsHide: true },
  );

  let buf = "";
  watcherProcess.stdout?.on("data", (d: Buffer) => {
    buf += d.toString();
    const lines = buf.split(/\r?\n/);
    buf = lines.pop() ?? "";
    for (const line of lines) {
      const m = line.trim().match(/^KILLED\s+(.+)$/i);
      if (!m) continue;
      const app = appByExe(m[1]!.trim());
      if (app) {
        console.log(`[focus-watcher] killed on launch: ${m[1]}`);
        notifyBlocked(app);
      }
    }
  });
  watcherProcess.stderr?.on("data", (d: Buffer) =>
    console.warn("[focus-watcher] ps stderr:", d.toString().slice(0, 200)),
  );
  watcherProcess.on("error", (e) => console.warn("[focus-watcher] spawn error:", e));
  watcherProcess.on("exit", () => {
    watcherProcess = null;
  });

  // Backup-sweep: гарантирует kill даже если WMI-события не приходят
  // (proven path — tasklist + taskkill). Раз в 1.5s.
  backupSweepTimer = setInterval(() => {
    void backupSweep();
  }, 1500);
}

function stopProcessWatcher(): void {
  if (watcherProcess) {
    try {
      watcherProcess.kill();
    } catch {
      /* ignore */
    }
    watcherProcess = null;
  }
  if (backupSweepTimer) {
    clearInterval(backupSweepTimer);
    backupSweepTimer = null;
  }
  try {
    unlinkSync(SNOOZE_FILE);
  } catch {
    /* ignore */
  }
}

function isProcessRunning(exeName: string): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn("tasklist", ["/FI", `IMAGENAME eq ${exeName}`, "/FO", "CSV", "/NH"], {
      windowsHide: true,
    });
    let out = "";
    proc.stdout?.on("data", (d: Buffer) => (out += d.toString()));
    proc.on("close", () => resolve(out.toLowerCase().includes(exeName.toLowerCase())));
    proc.on("error", () => resolve(false));
  });
}

function killExe(exeName: string): Promise<void> {
  return new Promise((resolve) => {
    const proc = spawn("taskkill", ["/F", "/IM", exeName], { windowsHide: true });
    proc.on("close", () => resolve());
    proc.on("error", () => resolve());
  });
}

async function backupSweep(): Promise<void> {
  for (const app of lastRawBlockedApps) {
    const exe = exeNameOf(app);
    if (!exe || isBlockedFocusAppSnoozed(app.id)) continue;
    if (!(await isProcessRunning(exe))) continue;
    await killExe(exe);
    console.log(`[focus-watcher] backup-killed: ${exe}`);
    notifyBlocked(app);
  }
}

// Однократный kill при старте фокуса — убиваем уже запущенные заблокированные.
async function killRunningBlockedApps(): Promise<void> {
  for (const app of lastRawBlockedApps) {
    const exe = exeNameOf(app);
    if (!exe || isBlockedFocusAppSnoozed(app.id)) continue;
    if (!(await isProcessRunning(exe))) continue;
    await killExe(exe);
    notifyBlocked(app);
  }
}

function normalizeBlockedApps(apps: FocusBlockedApp[]): FocusBlockedApp[] {
  const byId = new Map<string, FocusBlockedApp>();
  for (const app of apps) {
    const id = app.id.trim();
    const name = app.name.trim();
    if (!id || !name) continue;
    byId.set(id, { id, name, icon: app.icon ?? null });
  }
  return Array.from(byId.values());
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
  const blockedAppIds = Array.from(new Set(input.blockedAppIds?.filter(Boolean) ?? []));
  const rawBlockedApps = input.blockedApps ?? [];
  lastRawBlockedApps = rawBlockedApps;
  const blockedApps = normalizeBlockedApps(rawBlockedApps);
  if (input.mode === "allow") {
    throw new Error("Режим Allow требует app/browser-level блокировки и пока недоступен");
  }
  lastCategoryIds = categoryIds;
  lastBlockedAppIds = blockedAppIds;
  lastBlockedApps = blockedApps;
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
  await applyFocusState(
    categoryIds.length > 0 || blockedAppIds.length > 0 || blockedApps.length > 0,
    categoryIds,
    blockedAppIds,
    blockedApps,
  );
  if (rawBlockedApps.length > 0) {
    await killRunningBlockedApps();
  }
  startProcessWatcher();
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
    startProcessWatcher();
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
  stopProcessWatcher();
  lastRawBlockedApps = [];
  const state = await invoke<PomodoroState>("pomodoro.stop");
  setFocusState(deriveFocusWidgetPatch(state));
  await enqueueSideEffect(() => closeTimeEntry(false));
  await applyFocusState(false);
  lastCategoryIds = [];
  lastBlockedAppIds = [];
  lastBlockedApps = [];
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
ipcMain.handle("kepler:focus-session:snooze-app", (_e, appId: string) =>
  snoozeBlockedFocusApp(appId),
);

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
          stopProcessWatcher();
          lastRawBlockedApps = [];
          await applyFocusState(false);
          lastCategoryIds = [];
          lastBlockedAppIds = [];
          lastBlockedApps = [];
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
  stopProcessWatcher();
  pendingWorkCompletion = false;
}
