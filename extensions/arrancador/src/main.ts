// Arrancador Vue extension entry — Kepler shell host.
//
// Scope (Phase 4 → расширено Phase 4.5):
//   - UI порт всех страниц legacy Arrancador renderer'а (Library, Catalogue,
//     Scan, SQOBA, Statistics, Settings, GameDetail).
//   - Read-only через `kepler.ark.request("list_objects_by_type", ...)`.
//   - Write paths (game launch, scanner, backups, RAWG add) — Phase 5+.
//
// API:
//   - window.kepler.ark.request("list_objects_by_type", { type_id: "game_obj" })
//   - window.kepler.ark.subscribe("entity_changed", handler) — refresh on change
//
// Native `window.electronAPI.*` пути из legacy здесь недоступны.

import { createApp } from "vue";

// eslint-disable-next-line import/no-unassigned-import
import "@kepler/visuals/theme/css";
// eslint-disable-next-line import/no-unassigned-import
import "./styles.css";

import App from "./App.vue";
import { router } from "./router";

document.documentElement.classList.add("dark");

createApp(App).use(router).mount("#app");

// Deep links через IPC. См. extensions/delphi/src/main.ts для пояснения.
const keplerNav = (
  window as unknown as {
    kepler?: {
      navigation?: {
        initialRoute: () => Promise<string | null>;
        onNavigate: (h: (route: string) => void) => () => void;
      };
    };
  }
).kepler?.navigation;
if (keplerNav) {
  void keplerNav.initialRoute().then((r) => {
    if (r) void router.push(r);
  });
  keplerNav.onNavigate((r) => {
    void router.push(r);
  });
}
