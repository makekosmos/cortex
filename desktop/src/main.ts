import { createApp, defineAsyncComponent, h, ref } from "vue";
import { installConsoleOnlyRuntimeErrors, installScrollFadeListener } from "@kosmos/visuals";
import App from "./App.vue";
import SettingsView from "./views/SettingsView.vue";
import "./styles.css";

document.documentElement.classList.add("dark");

installScrollFadeListener();

// Hash-based dispatch: один renderer-bundle, несколько окон. Каждое окно
// грузит URL с разным hash, рендер ниже выбирает соответствующий root view.
//   (no hash)                  → legacy compatibility launcher (App.vue)
//   #settings                  → SettingsView
//   #/dashboard                → DashboardRoot (DashboardView)
const hash = ref(window.location.hash);
window.addEventListener("hashchange", () => {
  hash.value = window.location.hash;
});

function rootView() {
  const currentHash = hash.value;
  if (currentHash.startsWith("#settings")) return SettingsView;
  if (currentHash.startsWith("#/dashboard")) {
    // Async — dashboard views и их деревья не нужны для launcher / settings окон.
    return defineAsyncComponent(() => import("./views/DashboardRoot.vue"));
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
  // См. postmortems.md § 2026-07-31: compatibility BrowserWindow loads without a hash.
  return App;
}

// KOS-77: `db_restored` приходит из Engine после snapshot-restore через Core
// db_backup_restore. In-memory состояние всех data-view устарело — окно
// перезагружается и перечитывает ARK. Overlay-окна (focus widget / block
// overlay / dictation pill) не показывают ARK-данные и не перезагружаются,
// чтобы не срывать активную focus/dictation сессию.
const OVERLAY_ROOTS = ["#focus-widget", "#focus-block-overlay", "#dictation-pill"];
window.kepler?.ark?.onEvent?.((event) => {
  if (
    event?.event === "db_restored" &&
    !OVERLAY_ROOTS.some((root) => window.location.hash.startsWith(root))
  ) {
    window.location.reload();
  }
});

const app = createApp({
  render: () => h(rootView()),
});
installConsoleOnlyRuntimeErrors(app, { label: "desktop-renderer" });
app.mount("#app");
