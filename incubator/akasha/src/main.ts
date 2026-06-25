import { createApp } from "vue";
import { installConsoleOnlyRuntimeErrors, installScrollFadeListener } from "@kosmos/visuals";

import "@kosmos/visuals/theme/css";
import "./styles.css";

import App from "./App.vue";

document.documentElement.classList.add("dark");
installScrollFadeListener();

const app = createApp(App);
installConsoleOnlyRuntimeErrors(app, { label: "akasha-extension" });
app.mount("#app");
