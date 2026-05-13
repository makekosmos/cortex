import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/list" },
  { path: "/list", component: () => import("./views/ListView.vue"), name: "list" },
  { path: "/pomodoro", component: () => import("./views/PomodoroView.vue"), name: "pomodoro" },
  { path: "/settings", component: () => import("./views/SettingsView.vue"), name: "settings" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
