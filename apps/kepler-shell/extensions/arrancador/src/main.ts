// Arrancador Vue extension entry — Kepler shell host (Phase 4).
//
// Scope: UI-only port of the legacy game library. Native scanner и game launch
// остаются в legacy `apps/arrancador/` (Phase 5 work). Extension показывает
// только game_obj, уже присутствующие в ARK.
//
// API:
//   - window.kepler.ark.request("list_objects_by_type", { type_id: "game_obj" })
//   - window.kepler.ark.subscribe("entity_changed", handler) — refresh on change
//
// Native `window.electronAPI.*` пути из legacy здесь недоступны.

import { createApp } from "vue";

// eslint-disable-next-line import/no-unassigned-import
import "@kosmos/visuals/theme/css";
// eslint-disable-next-line import/no-unassigned-import
import "./styles.css";

import App from "./App.vue";

document.documentElement.classList.add("dark");

createApp(App).mount("#app");
