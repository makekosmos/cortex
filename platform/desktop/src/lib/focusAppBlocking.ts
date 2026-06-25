import type { FocusActiveState } from "@shared/ipc-types";

export function normalizeFocusAppName(name: string): string {
  return name.trim().toLocaleLowerCase("ru-RU").replace(/\s+/g, " ");
}

export function isAppBlockedByFocus(
  focus: FocusActiveState | null | undefined,
  appId: string,
  appName?: string | null,
) {
  if (!focus?.active) return false;
  if ((focus.blocked_app_ids ?? []).includes(appId)) return true;
  const normalized = appName ? normalizeFocusAppName(appName) : "";
  if (!normalized) return false;
  return (focus.blocked_apps ?? []).some((app) => normalizeFocusAppName(app.name) === normalized);
}
