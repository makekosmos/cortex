import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

const routes: RouteRecordRaw[] = [
  { path: "/", component: () => import("./views/HomeView.vue"), name: "home" },
  // `/settings` загружается ОТДЕЛЬНЫМ Electron BrowserWindow'ом (через IPC
  // `settings:open`), а не навигацией в основном окне. App.vue видит этот
  // route и рендерит только SettingsView без chrome.
  { path: "/settings", component: () => import("./views/SettingsView.vue"), name: "settings" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
