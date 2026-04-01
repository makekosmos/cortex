import { watch, ref } from "vue";
import { useLayoutStore } from "@/store/layout";

export function useSearch() {
  const layout = useLayoutStore();
  const pendingQuery = ref("");

  // Debounce pending query into committed query
  let debounceTimer: number | null = null;
  watch(pendingQuery, (val) => {
    if (debounceTimer) window.clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      layout.searchQuery = val;
    }, 300);
  });

  // Execute search when committed query changes
  watch(
    () => layout.searchQuery,
    async (query) => {
      if (!query.trim()) {
        layout.searchResults = [];
        return;
      }
      if (!window.api) return;
      try {
        layout.searchResults = await window.api.searchEntries(query);
      } catch (error) {
        console.error("Search error", error);
      }
    },
  );

  return { pendingQuery };
}
