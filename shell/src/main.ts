import { createApp, defineAsyncComponent, h } from "vue";
import App from "./App.vue";
import SettingsView from "./views/SettingsView.vue";
import "./styles.css";

document.documentElement.classList.add("dark");

// Hash-based dispatch: один renderer-bundle, несколько окон. Каждое окно
// грузит URL с разным hash, рендер ниже выбирает соответствующий root view.
//   (no hash)                  → launcher (App.vue → LauncherView)
//   #settings                  → SettingsView
//   #/dashboard                → DashboardRoot (DashboardView)
const hash = window.location.hash;

function rootView() {
  if (hash.startsWith("#settings")) return SettingsView;
  if (hash.startsWith("#/dashboard")) {
    // Async — dashboard views и их деревья не нужны для launcher / settings окон.
    const DashboardRoot = defineAsyncComponent(
      () => import("./views/DashboardRoot.vue"),
    );
    return DashboardRoot;
  }
  return App;
}

createApp({
  render: () => h(rootView()),
}).mount("#app");
