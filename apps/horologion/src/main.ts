import { createApp } from "vue";
import "@kosmos/visuals/theme/css";
import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

// Horologion по умолчанию использует тёмную тему kosmos-visuals.
// (Класс `.dark` объявлен в @kosmos/visuals/theme/css-variables.css.)
document.documentElement.classList.add("dark");

createApp(App).use(router).mount("#app");
