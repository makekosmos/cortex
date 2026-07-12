import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "@kosmos/visuals/theme/css";
import "./styles.css";

createApp(App).use(createPinia()).mount("#root");
