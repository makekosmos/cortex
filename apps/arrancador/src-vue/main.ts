import { createApp, vaporInteropPlugin } from "vue";
import { createPinia } from "pinia";
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

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(vaporInteropPlugin);
app.mount("#root");
