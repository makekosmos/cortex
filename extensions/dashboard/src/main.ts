// Dashboard Vue extension entry — Kepler shell host.
//
// Loads ARK usage analytics через `window.kepler.ark.request("get_usage_analytics", ...)`,
// shape совпадает с legacy DashboardSnapshot (минус поле status — host-managed
// chooseDatabase / resetDatabase / getPlatform недоступны extension'у).

import { createApp } from "vue";
import "@kosmos/visuals/theme/css";
import "./styles.css";
import App from "./App.vue";

document.documentElement.classList.add("dark");
document.body.classList.add("dark");

createApp(App).mount("#app");
