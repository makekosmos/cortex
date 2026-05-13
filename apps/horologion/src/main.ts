import { createApp } from "vue";
// Inter Variable — основной sans-шрифт на Windows/Linux. На macOS подхватится
// системный SF Pro раньше (см. --font-sans в @kepler/visuals/theme/css-variables.css).
import "@fontsource-variable/inter";
import "@kepler/visuals/theme/css";
import App from "./App.vue";
import { router } from "./router";
import "./styles.css";

// Horologion по умолчанию использует тёмную тему kepler-visuals.
// (Класс `.dark` объявлен в @kepler/visuals/theme/css-variables.css.)
document.documentElement.classList.add("dark");

createApp(App).use(router).mount("#app");
