import { createApp } from "vue";
import "@kepler/visuals/theme/css";
import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

// Horologion по умолчанию использует тёмную тему kepler-visuals.
// (Класс `.dark` объявлен в @kepler/visuals/theme/css-variables.css.)
document.documentElement.classList.add("dark");

createApp(App).use(router).mount("#app");
