// Arrancador extension router.
//
// Differences vs legacy `apps/arrancador/src-vue/router.ts`:
//   - history → `createMemoryHistory()` (как в Delphi / Horologion extension'ах);
//     extension renderer'у не нужен URL hash, BrowserWindow грузит file://.
//   - Все route'ы плоские (без вложенного LayoutPage children): корневой
//     `App.vue` рендерит shell + `<router-view />`.

import { createMemoryHistory, createRouter, type RouteRecordRaw } from "vue-router";

import CataloguePage from "./pages/CataloguePage.vue";
import GameDetailPage from "./pages/GameDetailPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";
import ScanPage from "./pages/ScanPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import SqobaPage from "./pages/SqobaPage.vue";
import StatisticsPage from "./pages/StatisticsPage.vue";

const routes: RouteRecordRaw[] = [
  { path: "/", component: LibraryPage },
  { path: "/catalogue", component: CataloguePage },
  { path: "/scan", component: ScanPage },
  { path: "/sqoba", component: SqobaPage },
  { path: "/stats", component: StatisticsPage },
  { path: "/settings", component: SettingsPage },
  { path: "/game/:id", component: GameDetailPage },
];

export const router = createRouter({
  history: createMemoryHistory(),
  routes,
});
