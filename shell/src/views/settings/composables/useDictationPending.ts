// Состояние очереди pending диктовок: list + retry/discard/retry_all + poll.
// Используется в DictationTab.vue для секции «Очередь диктовок».
//
// Backend контракт — см. `services/kepler-backend/src/dictation/host.rs`:
//   dictation.list_pending → { items: [{ uuid, createdAt, attempts, ... }] }
//   dictation.retry { uuid } → { uuid, started: true }
//   dictation.discard { uuid } → { uuid, discarded: true }
//   dictation.retry_all → { started: number }

import { onMounted, onUnmounted, ref } from "vue";

export interface PendingItem {
  uuid: string;
  createdAt: string;
  attempts: number;
  lastError: string | null;
  durationSec: number;
  wavBytes: number;
  language: string;
}

const POLL_INTERVAL_MS = 5_000;

export function useDictationPending() {
  const pendingItems = ref<PendingItem[]>([]);
  const pendingError = ref<string | null>(null);
  const pendingLoading = ref(false);
  const busyUuid = ref<string | null>(null);

  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function loadPending(): Promise<void> {
    pendingLoading.value = true;
    pendingError.value = null;
    try {
      const resp = (await window.kepler.ark.request("dictation.list_pending", {})) as {
        items?: PendingItem[];
      };
      pendingItems.value = Array.isArray(resp.items) ? resp.items : [];
    } catch (e) {
      pendingError.value = (e as Error)?.message ?? String(e);
      pendingItems.value = [];
    } finally {
      pendingLoading.value = false;
    }
  }

  async function retryPending(uuid: string): Promise<void> {
    busyUuid.value = uuid;
    try {
      await window.kepler.ark.request("dictation.retry", { uuid });
      // Дождёмся пару секунд и refresh — backend убирает item из очереди
      // на success, или повышает attempts на failure.
      setTimeout(() => void loadPending(), 1500);
    } catch (e) {
      pendingError.value = (e as Error)?.message ?? String(e);
    } finally {
      busyUuid.value = null;
    }
  }

  async function discardPending(uuid: string): Promise<void> {
    busyUuid.value = uuid;
    try {
      await window.kepler.ark.request("dictation.discard", { uuid });
      await loadPending();
    } catch (e) {
      pendingError.value = (e as Error)?.message ?? String(e);
    } finally {
      busyUuid.value = null;
    }
  }

  async function retryAllPending(): Promise<void> {
    try {
      await window.kepler.ark.request("dictation.retry_all", {});
      setTimeout(() => void loadPending(), 1500);
    } catch (e) {
      pendingError.value = (e as Error)?.message ?? String(e);
    }
  }

  function formatCreatedAt(iso: string): string {
    try {
      const d = new Date(iso);
      const dd = String(d.getDate()).padStart(2, "0");
      const mm = String(d.getMonth() + 1).padStart(2, "0");
      const hh = String(d.getHours()).padStart(2, "0");
      const mi = String(d.getMinutes()).padStart(2, "0");
      return `${dd}.${mm} ${hh}:${mi}`;
    } catch {
      return iso;
    }
  }

  function formatDuration(sec: number): string {
    if (sec < 60) return `${sec.toFixed(1)} сек`;
    const m = Math.floor(sec / 60);
    const s = Math.round(sec % 60);
    return `${m}:${String(s).padStart(2, "0")}`;
  }

  onMounted(() => {
    void loadPending();
    pollTimer = setInterval(() => {
      void loadPending();
    }, POLL_INTERVAL_MS);
  });

  onUnmounted(() => {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  });

  return {
    pendingItems,
    pendingError,
    pendingLoading,
    busyUuid,
    loadPending,
    retryPending,
    discardPending,
    retryAllPending,
    formatCreatedAt,
    formatDuration,
  };
}
