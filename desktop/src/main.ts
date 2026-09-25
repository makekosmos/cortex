import { createApp, defineAsyncComponent, h, ref } from "vue";
import { installConsoleOnlyRuntimeErrors, installScrollFadeListener } from "@kosmos/visuals";
import SettingsView from "./views/SettingsView.vue";
import "./styles.css";

document.documentElement.classList.add("dark");

installScrollFadeListener();

// Hash-based dispatch: один renderer-bundle, несколько окон. Каждое окно
// грузит URL с разным hash, рендер ниже выбирает соответствующий root view.
//   (no hash)                  → пустой stub (headless test harness window)
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
    // Async — dashboard views и их деревья не нужны для settings / overlay окон.
    return defineAsyncComponent(() => import("./views/DashboardRoot.vue"));
  }
  if (currentHash.startsWith("#dictation-pill")) {
    // Дикта-pill — overlay с waveform + таймером во время записи.
    // Async — audio capture / encoding в settings bundle не нужны.
    return defineAsyncComponent(() => import("./views/DictationPillView.vue"));
  }
  // No-hash fallback: только headless test harness (main-test-window.ts)
  // грузит index.html без hash. Пользовательских окон без hash не осталось.
  return "div";
}

// KOS-77: `db_restored` приходит из Engine после snapshot-restore через Core
// db_backup_restore. In-memory состояние всех data-view устарело — окно
// перезагружается и перечитывает ARK. Overlay-окна (dictation pill) не
// показывают ARK-данные и не перезагружаются, чтобы не срывать активную
// dictation сессию.
const OVERLAY_ROOTS = ["#dictation-pill"];
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
