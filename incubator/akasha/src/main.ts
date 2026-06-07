import { createApp } from "vue";
import { installScrollFadeListener } from "@kosmos/visuals";

import "@kosmos/visuals/theme/css";
import "./styles.css";

import App from "./App.vue";

document.documentElement.classList.add("dark");
installScrollFadeListener();

createApp(App).mount("#app");
