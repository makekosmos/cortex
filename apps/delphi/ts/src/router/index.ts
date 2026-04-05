import type { RouteRecordRaw } from "vue-router";

export const routes: RouteRecordRaw[] = [
  { path: "/", component: () => import("../pages/AllTaskPage.vue") },

  { path: "/today", component: () => import("../pages/TodayPage.vue") },

  { path: "/upcoming", component: () => import("../pages/UpcomingPage.vue") },

  { path: "/logbook", component: () => import("../pages/LogbookPage.vue") },

  { path: "/trash", component: () => import("../pages/TrashPage.vue") },

  { path: "/settings", component: () => import("../pages/SettingsPage.vue") },

  { path: "/project/:id", component: () => import("../pages/ProjectPage.vue") },
];
