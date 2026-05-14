import { computed, readonly, ref, shallowRef } from "vue";
import type {
  UsageAnalyticsSnapshot,
  UsageAnalyticsOptions,
} from "@/types/analytics";

const AUTO_REFRESH_MS = 5_000;

const snapshot = ref<UsageAnalyticsSnapshot | null>(null);
const rangeDays = shallowRef(21);
const isLoading = shallowRef(false);
const error = shallowRef<string | null>(null);
const initialized = shallowRef(false);
const isWindowFocused = shallowRef(true);
const listenersBound = shallowRef(false);

let refreshTimer: ReturnType<typeof setInterval> | null = null;

function canUseDom(): boolean {
  return typeof window !== "undefined" && typeof document !== "undefined";
}

function shouldAutoRefresh(): boolean {
  if (!canUseDom()) {
    return false;
  }

  return (
    initialized.value &&
    isWindowFocused.value &&
    document.visibilityState === "visible"
  );
}

function stopAutoRefresh() {
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer);
    refreshTimer = null;
  }
}

function syncAutoRefresh() {
  if (!canUseDom()) {
    return;
  }

  if (!shouldAutoRefresh()) {
    stopAutoRefresh();
    return;
  }

  if (refreshTimer !== null) {
    return;
  }

  refreshTimer = setInterval(() => {
    void loadSnapshot();
  }, AUTO_REFRESH_MS);
}

async function loadSnapshot() {
  if (isLoading.value) {
    return;
  }

  if (!window.kepler) {
    error.value =
      "Extension bridge (window.kepler) недоступен — Kepler shell не загрузил preload.";
    return;
  }

  isLoading.value = true;
  error.value = null;

  try {
    const options: UsageAnalyticsOptions = {
      rangeDays: rangeDays.value,
      topAppsLimit: 8,
      recentSessionsLimit: 24,
    };
    snapshot.value = await window.kepler.ark.request<UsageAnalyticsSnapshot>(
      "get_usage_analytics",
      options as unknown as Record<string, unknown>,
    );
  } catch (loadError) {
    error.value = (loadError as Error).message;
  } finally {
    isLoading.value = false;
  }
}

async function setRange(nextRange: number) {
  rangeDays.value = nextRange;
  await loadSnapshot();
}

function bindWindowListeners() {
  if (!canUseDom() || listenersBound.value) {
    return;
  }

  const handleFocus = () => {
    isWindowFocused.value = true;
    syncAutoRefresh();
    void loadSnapshot();
  };

  const handleBlur = () => {
    isWindowFocused.value = false;
    syncAutoRefresh();
  };

  const handleVisibilityChange = () => {
    isWindowFocused.value = document.hasFocus();
    syncAutoRefresh();
    if (shouldAutoRefresh()) {
      void loadSnapshot();
    }
  };

  isWindowFocused.value = document.hasFocus();
  window.addEventListener("focus", handleFocus);
  window.addEventListener("blur", handleBlur);
  document.addEventListener("visibilitychange", handleVisibilityChange);
  listenersBound.value = true;
}

async function initialize() {
  if (initialized.value) {
    syncAutoRefresh();
    return;
  }

  initialized.value = true;
  bindWindowListeners();
  await loadSnapshot();
  syncAutoRefresh();
}

export function useDashboardData() {
  const hasData = computed(
    () => (snapshot.value?.summary.sessionCount ?? 0) > 0,
  );

  return {
    snapshot: readonly(snapshot),
    rangeDays: readonly(rangeDays),
    isLoading: readonly(isLoading),
    error: readonly(error),
    hasData,
    initialize,
    loadSnapshot,
    setRange,
  };
}
