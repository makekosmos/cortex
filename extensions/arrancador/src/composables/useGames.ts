// useGames — простой shared store на vue refs (без Pinia).
//
// Один модуль-singleton для всех страниц. Подписывается на
// `entity_changed` через kepler.ark и автоматически обновляет список.

import { computed, onBeforeUnmount, onMounted, shallowRef, ref } from "vue";

import { arkBridge, loadGames, type ArrancadorGame } from "../lib/arkGames";

const games = shallowRef<ArrancadorGame[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
let refCount = 0;
let unsubscribe: (() => void) | null = null;
let initialLoadStarted = false;

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    games.value = await loadGames();
  } catch (cause) {
    // eslint-disable-next-line no-console
    console.error("[arrancador-extension] loadGames failed:", cause);
    error.value =
      cause instanceof Error ? cause.message : "Не удалось загрузить игры";
  } finally {
    loading.value = false;
  }
}

export function useGames() {
  onMounted(() => {
    refCount++;
    if (refCount === 1) {
      const bridge = arkBridge();
      if (bridge) {
        unsubscribe = bridge.subscribe("entity_changed", () => {
          void refresh();
        });
      }
    }
    if (!initialLoadStarted) {
      initialLoadStarted = true;
      void refresh();
    }
  });

  onBeforeUnmount(() => {
    refCount--;
    if (refCount === 0) {
      unsubscribe?.();
      unsubscribe = null;
    }
  });

  return {
    games,
    loading,
    error,
    refresh,
    findGame: (id: string) =>
      computed(() => games.value.find((g) => g.id === id) ?? null),
  };
}
