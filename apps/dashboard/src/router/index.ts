import type { RouteRecordRaw } from "vue-router";
import OverviewPage from "@/pages/OverviewPage.vue";
import SessionsPage from "@/pages/SessionsPage.vue";

export const routes: RouteRecordRaw[] = [
  {
    path: "/",
    redirect: "/overview",
  },
  {
    path: "/overview",
    name: "overview",
    component: OverviewPage,
  },
  {
    path: "/sessions",
    name: "sessions",
    component: SessionsPage,
  },
];
