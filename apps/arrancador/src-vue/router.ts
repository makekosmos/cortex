import type { RouteRecordRaw } from "vue-router";
import { createRouter, createWebHashHistory } from "vue-router";

const routes: RouteRecordRaw[] = [
  {
    path: "/",
    component: () => import("./pages/LayoutPage.vue"),
    children: [
      {
        path: "",
        component: () => import("./pages/LibraryPage.vue"),
      },
      {
        path: "catalogue",
        component: () => import("./pages/CataloguePage.vue"),
      },
      {
        path: "game/:id",
        component: () => import("./pages/GameDetailPage.vue"),
      },
      {
        path: "scan",
        component: () => import("./pages/ScanPage.vue"),
      },
      {
        path: "sqoba",
        component: () => import("./pages/SqobaPage.vue"),
      },
      {
        path: "statistics",
        component: () => import("./pages/StatisticsPage.vue"),
      },
      {
        path: "system",
        redirect: "/",
      },
      {
        path: "achievements",
        redirect: "/",
      },
      {
        path: "settings",
        component: () => import("./pages/SettingsPage.vue"),
      },
    ],
  },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
