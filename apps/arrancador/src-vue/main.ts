import { createPinia } from "pinia";
import { createApp, vaporInteropPlugin } from "vue";
import App from "./App.vue";
import { initializeLanguage } from "./composables/useLanguage";
import { initializeSidebarConfig } from "./composables/useSidebarConfig";
import { initializeTheme } from "./composables/useTheme";
import { initializeToastBridge } from "./composables/useToast";
import { router } from "./router";
import "../src/index.css";

initializeTheme();
initializeLanguage();
initializeSidebarConfig();
initializeToastBridge();

const startupScreen =
  typeof document === "undefined"
    ? null
    : document.getElementById("arrancador-startup-screen");

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(vaporInteropPlugin);
app.mount("#root");

startupScreen?.remove();
