import { createApp } from "vue";
import App from "./App.vue";
import SettingsView from "./views/SettingsView.vue";
import "./styles.css";

document.documentElement.classList.add("dark");

// Hash-based dispatch: один renderer-bundle, два окна. SettingsWindow
// грузит URL c #settings, LauncherWindow — без hash.
const isSettings = window.location.hash.startsWith("#settings");

createApp(isSettings ? SettingsView : App).mount("#app");
