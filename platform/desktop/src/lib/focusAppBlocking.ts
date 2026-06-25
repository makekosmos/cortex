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
  const blockedAppIds = focus.blocked_app_ids;
  if (Array.isArray(blockedAppIds) && blockedAppIds.includes(appId)) return true;
  const normalized = appName ? normalizeFocusAppName(appName) : "";
  if (!normalized) return false;
  const blockedApps = focus.blocked_apps;
  return (
    Array.isArray(blockedApps) &&
    blockedApps.some((app) => normalizeFocusAppName(app.name) === normalized)
  );
}
