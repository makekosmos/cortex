import type { FocusBlockedApp, PomodoroState } from "./focus-session-types";
import type { FocusState } from "./focus-widget";

// SAFETY: this shared empty list is intentionally frozen and only returned as a read-only fallback.
const EMPTY_BLOCKED_APPS: FocusBlockedApp[] = [];
Object.freeze(EMPTY_BLOCKED_APPS);

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

export function normalizeBlockedAppsOrEmpty(
  apps: FocusBlockedApp[] | undefined,
): FocusBlockedApp[] {
  return apps ? normalizeBlockedApps(apps) : [];
}

export function blockedAppsOrEmpty(apps: FocusBlockedApp[] | undefined): FocusBlockedApp[] {
  return apps ?? EMPTY_BLOCKED_APPS;
}

export function compactStrings(values: string[] | undefined): string[] {
  return values ? values.filter(Boolean) : [];
}

export function deriveFocusWidgetPatch(raw: PomodoroState): Partial<FocusState> {
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
