import { createApp, vaporInteropPlugin } from "vue";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import { routes } from "./router";
import "./global.css";

document.documentElement.classList.add("dark");
document.body.classList.add("dark");

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

const app = createApp(App);
app.use(router);
app.use(vaporInteropPlugin);
app.mount("#root");
