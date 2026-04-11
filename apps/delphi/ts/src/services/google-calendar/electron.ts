import type {
  GoogleCalendarRange,
  GoogleCalendarSnapshot,
} from "@/services/google-calendar/contracts";

function invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
  if (!window.electronAPI?.invoke) {
    throw new Error("Google Calendar integration is available only in Electron.");
  }

  return window.electronAPI.invoke(channel, ...args) as Promise<T>;
}

export function isGoogleCalendarSupported(): boolean {
  return Boolean(window.electronAPI?.invoke);
}

export async function getGoogleCalendarSnapshot(): Promise<GoogleCalendarSnapshot> {
  return invoke<GoogleCalendarSnapshot>("google-calendar:getSnapshot");
}

export async function saveGoogleCalendarConfig(config: {
  clientId: string;
  clientSecret: string;
}): Promise<GoogleCalendarSnapshot> {
  return invoke<GoogleCalendarSnapshot>("google-calendar:setConfig", config);
}

export async function connectGoogleCalendar(): Promise<GoogleCalendarSnapshot> {
  return invoke<GoogleCalendarSnapshot>("google-calendar:signIn");
}

export async function disconnectGoogleCalendar(): Promise<GoogleCalendarSnapshot> {
  return invoke<GoogleCalendarSnapshot>("google-calendar:signOut");
}

export async function refreshGoogleCalendar(
  range?: GoogleCalendarRange,
  force = false,
): Promise<GoogleCalendarSnapshot> {
  return invoke<GoogleCalendarSnapshot>("google-calendar:refresh", range, force);
}
