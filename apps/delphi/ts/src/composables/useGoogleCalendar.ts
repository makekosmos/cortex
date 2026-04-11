import { computed, readonly, shallowRef } from "vue";
import {
  connectGoogleCalendar,
  disconnectGoogleCalendar,
  getGoogleCalendarSnapshot,
  isGoogleCalendarSupported,
  refreshGoogleCalendar,
  saveGoogleCalendarConfig,
} from "@/services/google-calendar/electron";
import {
  EMPTY_GOOGLE_CALENDAR_SNAPSHOT,
  type GoogleCalendarRange,
} from "@/services/google-calendar/contracts";

const snapshot = shallowRef(EMPTY_GOOGLE_CALENDAR_SNAPSHOT);
const loading = shallowRef(false);
const loadError = shallowRef<string | null>(null);
let inFlightLoad: Promise<typeof EMPTY_GOOGLE_CALENDAR_SNAPSHOT> | null = null;

async function runAndStore(
  action: () => Promise<typeof EMPTY_GOOGLE_CALENDAR_SNAPSHOT>,
): Promise<typeof EMPTY_GOOGLE_CALENDAR_SNAPSHOT> {
  loading.value = true;
  loadError.value = null;

  try {
    const next = await action();
    snapshot.value = next;
    return next;
  } catch (error) {
    loadError.value =
      error instanceof Error ? error.message : "Не удалось обновить календарь.";
    throw error;
  } finally {
    loading.value = false;
  }
}

async function load() {
  if (!isGoogleCalendarSupported()) {
    snapshot.value = {
      ...EMPTY_GOOGLE_CALENDAR_SNAPSHOT,
      status: {
        ...EMPTY_GOOGLE_CALENDAR_SNAPSHOT.status,
        available: false,
      },
    };
    return;
  }

  if (!inFlightLoad) {
    inFlightLoad = runAndStore(() => getGoogleCalendarSnapshot()).finally(() => {
      inFlightLoad = null;
    });
  }

  return inFlightLoad;
}

export function useGoogleCalendar() {
  return {
    snapshot: readonly(snapshot),
    status: computed(() => snapshot.value.status),
    account: computed(() => snapshot.value.account),
    calendars: computed(() => snapshot.value.calendars),
    events: computed(() => snapshot.value.events),
    config: computed(() => snapshot.value.config),
    loading: readonly(loading),
    loadError: readonly(loadError),
    isSupported: isGoogleCalendarSupported,
    load,
    saveConfig: (config: { clientId: string; clientSecret: string }) =>
      runAndStore(() => saveGoogleCalendarConfig(config)),
    connect: () => runAndStore(() => connectGoogleCalendar()),
    disconnect: () => runAndStore(() => disconnectGoogleCalendar()),
    refresh: (range?: GoogleCalendarRange, force = false) =>
      runAndStore(() => refreshGoogleCalendar(range, force)),
  };
}
