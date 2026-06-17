import { createApp, defineAsyncComponent, h, ref } from "vue";
import { installScrollFadeListener } from "@kosmos/visuals";
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
    const InstallExtensionView = defineAsyncComponent(
      () => import("./views/InstallExtensionView.vue"),
    );
    return InstallExtensionView;
  }
  if (currentHash.startsWith("#/dashboard")) {
    // Async — dashboard views и их деревья не нужны для launcher / settings окон.
    const DashboardRoot = defineAsyncComponent(() => import("./views/DashboardRoot.vue"));
    return DashboardRoot;
  }
  if (currentHash.startsWith("#raycast-host") || currentHash.startsWith("#command-host")) {
    const RaycastHostView = defineAsyncComponent(() => import("./views/RaycastHostView.vue"));
    return RaycastHostView;
  }
  if (currentHash.startsWith("#focus-widget")) {
    // Tiny always-on-top widget для активной pomodoro сессии. Async чтобы
    // не тащить в launcher bundle.
    const FocusWidgetView = defineAsyncComponent(() => import("./views/FocusWidgetView.vue"));
    return FocusWidgetView;
  }
  if (currentHash.startsWith("#focus-block-overlay")) {
    const FocusBlockOverlay = defineAsyncComponent(() => import("./views/FocusBlockOverlay.vue"));
    return FocusBlockOverlay;
  }
  if (currentHash.startsWith("#dictation-pill")) {
    // Дикта-pill — overlay с waveform + таймером во время записи.
    // Async — audio capture / encoding в launcher bundle не нужны.
    const DictationPillView = defineAsyncComponent(() => import("./views/DictationPillView.vue"));
    return DictationPillView;
  }
  return App;
}

createApp({
  render: () => h(rootView()),
}).mount("#app");
