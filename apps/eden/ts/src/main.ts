import { createApp, vaporInteropPlugin } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./index.css";

createApp(App).use(createPinia()).use(vaporInteropPlugin).mount("#root");
