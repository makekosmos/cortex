// Глобальный поисковый запрос — shared между AppSpotlight (top bar) и
// LibraryPage / CataloguePage. Простой module-level ref, не Pinia.

import { ref } from "vue";

const query = ref("");

export function useSearchQuery() {
  return query;
}
