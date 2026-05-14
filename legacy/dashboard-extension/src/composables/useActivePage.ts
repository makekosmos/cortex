// Простейший локальный "роутер" — без vue-router зависимости. Extension
// имеет всего две страницы (overview / sessions), полноценный history не нужен.

import { ref } from "vue";

export type DashboardPage = "overview" | "sessions";

const activePage = ref<DashboardPage>("overview");

export function useActivePage() {
  return {
    activePage,
    setPage(next: DashboardPage) {
      activePage.value = next;
    },
  };
}
