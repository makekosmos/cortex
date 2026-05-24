import { createApp, defineAsyncComponent, h } from "vue";
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
const hash = window.location.hash;

function rootView() {
  if (hash.startsWith("#settings")) return SettingsView;
  if (hash.startsWith("#install-extension")) {
    const InstallExtensionView = defineAsyncComponent(
      () => import("./views/InstallExtensionView.vue"),
    );
    return InstallExtensionView;
  }
  if (hash.startsWith("#/dashboard")) {
    // Async — dashboard views и их деревья не нужны для launcher / settings окон.
    const DashboardRoot = defineAsyncComponent(() => import("./views/DashboardRoot.vue"));
    return DashboardRoot;
  }
  if (hash.startsWith("#focus-widget")) {
    // Tiny always-on-top widget для активной pomodoro сессии. Async чтобы
    // не тащить в launcher bundle.
    const FocusWidgetView = defineAsyncComponent(() => import("./views/FocusWidgetView.vue"));
    return FocusWidgetView;
  }
  if (hash.startsWith("#dictation-pill")) {
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
