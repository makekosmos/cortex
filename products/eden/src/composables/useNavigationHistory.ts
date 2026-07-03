// useNavigationHistory — back/forward stack для Eden's activeScreen +
// currentEntryId + activeSpace. Snapshots сохраняются
// при каждом изменении `currentHistorySnapshot`, кроме случаев когда сам
// `applyHistorySnapshot` мутирует store ("suppressHistoryRecording").
//
// Logic-block, который раньше жил в App.vue (~150 строк): MAX_HISTORY
// = 30 (slice tail), `appendHistorySnapshot`, `snapshotsEqual`,
// `applyHistorySnapshot` async loader + `navigateBack/Forward` + watch.

import { computed, nextTick, shallowRef, watch, type ComputedRef } from "vue";
import type { useEdenStore } from "@/store/eden";
import type { SpaceId } from "@/components/sidebar/types";
import type { ActiveScreen } from "@/store/edenStoreHelpers";

const MAX_NAVIGATION_HISTORY = 30;

export interface EdenHistorySnapshot {
  activeScreen: ActiveScreen;
  currentEntryId: string | null;
  activeSpace: SpaceId;
}

function appendHistorySnapshot(
  snapshots: EdenHistorySnapshot[],
  snapshot: EdenHistorySnapshot,
): EdenHistorySnapshot[] {
  const next = [...snapshots, snapshot];
  if (next.length <= MAX_NAVIGATION_HISTORY) {
    return next;
  }
  return next.slice(next.length - MAX_NAVIGATION_HISTORY);
}

function snapshotsEqual(a: EdenHistorySnapshot | null, b: EdenHistorySnapshot | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.activeScreen === b.activeScreen &&
    a.currentEntryId === b.currentEntryId &&
    a.activeSpace === b.activeSpace
  );
}

export function useNavigationHistory(eden: ReturnType<typeof useEdenStore>) {
  const backStack = shallowRef<EdenHistorySnapshot[]>([]);
  const forwardStack = shallowRef<EdenHistorySnapshot[]>([]);
  const historyReady = shallowRef(false);
  const suppressHistoryRecording = shallowRef(false);

  const currentHistorySnapshot: ComputedRef<EdenHistorySnapshot | null> = computed(() => {
    if (eden.isInitializing || !eden.vaultPath) return null;
    return {
      activeScreen: eden.activeScreen,
      currentEntryId: eden.currentEntry?.id ?? null,
      activeSpace: eden.activeSpace,
    };
  });

  const canGoBack = computed(() => backStack.value.length > 0);
  const canGoForward = computed(() => forwardStack.value.length > 0);

  async function applyHistorySnapshot(snapshot: EdenHistorySnapshot) {
    suppressHistoryRecording.value = true;
    try {
      eden.activeSpace = snapshot.activeSpace;
      eden.activeScreen = snapshot.activeScreen;

      if (!snapshot.currentEntryId) {
        eden.currentEntry = null;
        return;
      }

      if (!window.api) {
        eden.currentEntry = null;
        return;
      }

      await eden.navigateTo(snapshot.currentEntryId);
    } finally {
      await nextTick();
      suppressHistoryRecording.value = false;
    }
  }

  async function navigateBack() {
    const targetSnapshot = backStack.value.at(-1);
    const currentSnapshot = currentHistorySnapshot.value;
    if (!targetSnapshot || !currentSnapshot) return;

    backStack.value = backStack.value.slice(0, -1);
    forwardStack.value = appendHistorySnapshot(forwardStack.value, currentSnapshot);
    await applyHistorySnapshot(targetSnapshot);
  }

  async function navigateForward() {
    const targetSnapshot = forwardStack.value.at(-1);
    const currentSnapshot = currentHistorySnapshot.value;
    if (!targetSnapshot || !currentSnapshot) return;

    forwardStack.value = forwardStack.value.slice(0, -1);
    backStack.value = appendHistorySnapshot(backStack.value, currentSnapshot);
    await applyHistorySnapshot(targetSnapshot);
  }

  watch(currentHistorySnapshot, (nextSnapshot, previousSnapshot) => {
    if (!nextSnapshot) return;

    if (!historyReady.value) {
      historyReady.value = true;
      return;
    }

    if (
      suppressHistoryRecording.value ||
      !previousSnapshot ||
      snapshotsEqual(nextSnapshot, previousSnapshot)
    ) {
      return;
    }

    backStack.value = appendHistorySnapshot(backStack.value, previousSnapshot);
    forwardStack.value = [];
  });

  return {
    canGoBack,
    canGoForward,
    navigateBack,
    navigateForward,
  };
}
