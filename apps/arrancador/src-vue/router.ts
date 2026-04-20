import type { RouteRecordRaw } from "vue-router";
import { createRouter, createWebHashHistory } from "vue-router";
import LayoutPage from "./pages/LayoutPage.vue";
import AchievementsPage from "./pages/AchievementsPage.vue";
import CataloguePage from "./pages/CataloguePage.vue";
import GameDetailPage from "./pages/GameDetailPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";
import ScanPage from "./pages/ScanPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import SqobaPage from "./pages/SqobaPage.vue";
import StatisticsPage from "./pages/StatisticsPage.vue";
import SystemInfoPage from "./pages/SystemInfoPage.vue";

const routes: RouteRecordRaw[] = [
  {
    path: "/",
    component: LayoutPage,
    children: [
      {
        path: "",
        component: LibraryPage,
      },
      {
        path: "catalogue",
        component: CataloguePage,
      },
      {
        path: "achievements",
        component: AchievementsPage,
      },
      {
        path: "game/:id",
        component: GameDetailPage,
      },
      {
        path: "scan",
        component: ScanPage,
      },
      {
        path: "sqoba",
        component: SqobaPage,
      },
      {
        path: "statistics",
        component: StatisticsPage,
      },
      {
        path: "system",
        component: SystemInfoPage,
      },
      {
        path: "settings",
        component: SettingsPage,
      },
    ],
  },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
