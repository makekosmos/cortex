import { createApp, defineAsyncComponent, h, ref } from "vue";
import { installConsoleOnlyRuntimeErrors, installScrollFadeListener } from "@kosmos/visuals";
import App from "./App.vue";
import SettingsView from "./views/SettingsView.vue";
import "./styles.css";

document.documentElement.classList.add("dark");

installScrollFadeListener();

// Hash-based dispatch: один renderer-bundle, несколько окон. Каждое окно
// грузит URL с разным hash, рендер ниже выбирает соответствующий root view.
//   (no hash)                  → launcher (App.vue → LauncherView)
//   #settings                  → SettingsView
//   #/dashboard                → DashboardRoot (DashboardView)
const hash = ref(window.location.hash);
window.addEventListener("hashchange", () => {
  hash.value = window.location.hash;
});

function rootView() {
  const currentHash = hash.value;
  if (currentHash.startsWith("#settings")) return SettingsView;
  if (currentHash.startsWith("#install-extension")) {
    return defineAsyncComponent(() => import("./views/InstallExtensionView.vue"));
  }
  if (currentHash.startsWith("#/dashboard")) {
    // Async — dashboard views и их деревья не нужны для launcher / settings окон.
    return defineAsyncComponent(() => import("./views/DashboardRoot.vue"));
  }
  if (currentHash.startsWith("#/my-cosmos")) {
    // Async — cosmos.gl + граф не нужны в launcher / settings бандлах.
    return defineAsyncComponent(() => import("./my-cosmos/MyCosmosView.vue"));
  }
  if (currentHash.startsWith("#command-host")) {
    return defineAsyncComponent(() => import("./views/CommandHostView.vue"));
  }
  if (currentHash.startsWith("#focus-widget")) {
    // Tiny always-on-top widget для активной pomodoro сессии. Async чтобы
    // не тащить в launcher bundle.
    return defineAsyncComponent(() => import("./views/FocusWidgetView.vue"));
  }
  if (currentHash.startsWith("#focus-block-overlay")) {
    return defineAsyncComponent(() => import("./views/FocusBlockOverlay.vue"));
  }
  if (currentHash.startsWith("#dictation-pill")) {
    // Дикта-pill — overlay с waveform + таймером во время записи.
    // Async — audio capture / encoding в launcher bundle не нужны.
    return defineAsyncComponent(() => import("./views/DictationPillView.vue"));
  }
  return App;
}

const app = createApp({
  render: () => h(rootView()),
});
installConsoleOnlyRuntimeErrors(app, { label: "desktop-renderer" });
app.mount("#app");
