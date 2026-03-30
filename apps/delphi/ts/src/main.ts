import { createApp } from "vue";
import { createPinia } from "pinia";
import {
  createRouter,
  createMemoryHistory,
  createWebHistory,
} from "vue-router";
import App from "./App.vue";
import { routes } from "./router";
import { isElectronRuntime } from "@/services/runtime/platform";
// eslint-disable-next-line import/no-unassigned-import
import "./global.css";
// eslint-disable-next-line import/no-unassigned-import
import "./composables/useTheme"; // apply saved theme before first paint

const router = createRouter({
  history: isElectronRuntime() ? createMemoryHistory() : createWebHistory(),
  routes,
});

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.mount("#root");
