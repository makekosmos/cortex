import { createApp } from "vue";
import App from "./App.vue";
import "@kosmos/visuals/theme/css";
import "./styles.css";

document.documentElement.classList.add("dark");
createApp(App).mount("#app");
