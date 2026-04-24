import { computed, shallowRef, watch } from "vue";
import { gamesApi } from "@/lib/api";
import type { NewGameProcessBinding, UsageProcessCandidate } from "@/types";

interface UseUsageProcessPickerOptions {
  isOpen: () => boolean;
  debounceMs?: number;
  limit?: number;
}

export function useUsageProcessPicker(options: UseUsageProcessPickerOptions) {
  const query = shallowRef("");
  const items = shallowRef<UsageProcessCandidate[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  const selectedCandidates = shallowRef<Record<string, UsageProcessCandidate>>({});
  const suppressNextBlankQuery = shallowRef(false);
  const debounceMs = options.debounceMs ?? 250;
  const limit = options.limit ?? 10;
  let activeRequestId = 0;

  const selectedBindings = computed<NewGameProcessBinding[]>(() => {
    const deduped = new Set<string>();
    return Object.values(selectedCandidates.value)
      .map((item) => ({
        match_type: item.binding_match_type,
        match_value: item.binding_match_value,
      }))
      .filter((binding) => {
        const key = `${binding.match_type}:${binding.match_value.trim().toLowerCase()}`;
        if (deduped.has(key)) {
          return false;
        }
        deduped.add(key);
        return true;
      });
  });

  const selectedCount = computed(() => Object.keys(selectedCandidates.value).length);

  const runRequest = async (
    loader: () => Promise<UsageProcessCandidate[]>,
    failureMessage: string,
  ) => {
    const requestId = ++activeRequestId;
    loading.value = true;
    error.value = null;

    try {
      const nextItems = await loader();
      if (!options.isOpen() || requestId !== activeRequestId) {
        return;
      }
      items.value = nextItems;
    } catch (cause) {
      if (!options.isOpen() || requestId !== activeRequestId) {
        return;
      }
      console.error(failureMessage, cause);
      error.value = failureMessage;
      items.value = [];
    } finally {
      if (requestId === activeRequestId) {
        loading.value = false;
      }
    }
  };

  const loadRecent = async () =>
    await runRequest(
      async () => await gamesApi.listRecentUsageProcesses(limit),
      "Не удалось загрузить последние процессы",
    );

  const searchProcesses = async (value: string) => {
    await runRequest(
      async () => await gamesApi.searchUsageProcesses(value, limit),
      "Не удалось выполнить поиск по usage tracker",
    );
  };

  watch(
    () => options.isOpen(),
    (isOpen) => {
      activeRequestId += 1;

      if (!isOpen) {
        query.value = "";
        items.value = [];
        error.value = null;
        selectedCandidates.value = {};
        loading.value = false;
        return;
      }

      suppressNextBlankQuery.value = true;
      query.value = "";
      items.value = [];
      error.value = null;
      selectedCandidates.value = {};
      void loadRecent();
    },
    { immediate: true },
  );

  watch(
    query,
    (nextQuery, _previousQuery, onCleanup) => {
      if (!options.isOpen()) {
        return;
      }

      const trimmed = nextQuery.trim();
      if (!trimmed && suppressNextBlankQuery.value) {
        suppressNextBlankQuery.value = false;
        return;
      }

      const timer = setTimeout(() => {
        if (!trimmed) {
          void loadRecent();
          return;
        }
        void searchProcesses(trimmed);
      }, debounceMs);

      onCleanup(() => {
        clearTimeout(timer);
      });
    },
  );

  const isSelected = (trackedAppId: string) => trackedAppId in selectedCandidates.value;

  const toggleSelection = (candidate: UsageProcessCandidate) => {
    const next = { ...selectedCandidates.value };
    if (next[candidate.tracked_app_id]) {
      delete next[candidate.tracked_app_id];
    } else {
      next[candidate.tracked_app_id] = candidate;
    }
    selectedCandidates.value = next;
  };

  return {
    query,
    items,
    loading,
    error,
    selectedBindings,
    selectedCount,
    isSelected,
    toggleSelection,
  };
}
