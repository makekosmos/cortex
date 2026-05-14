import { createApp } from "vue";
// Inter Variable — основной sans-шрифт на Windows/Linux. На macOS подхватится
// системный SF Pro раньше (см. --font-sans в @kosmos/visuals/theme/css-variables.css).
import "@fontsource-variable/inter";
import "@kosmos/visuals/theme/css";
import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

// Horologion по умолчанию использует тёмную тему kosmos-visuals.
// (Класс `.dark` объявлен в @kosmos/visuals/theme/css-variables.css.)
document.documentElement.classList.add("dark");

createApp(App).use(router).mount("#app");
