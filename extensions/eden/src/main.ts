// Eden Vue extension entry — Kepler shell host.
//
// Архитектурные отличия от standalone `apps/eden/ts/src/main.ts`:
//   1. Нет собственного Electron main процесса — `window.api` эмулируется
//      shim'ом (`./lib/kepler-api-shim`), который роутит ARK-операции через
//      `window.kepler.ark.request(operation, params)`.
//   2. Action-команды (`eden:note:create`, `eden:note:open-today`) — это
//      static open-команды Kepler shell'а (см. `shell/electron/commands.ts`).
//      Они вызывают `openAsExtension("eden", "/today" | "/new")` —
//      Eden получает initialRoute через `kepler.navigation.initialRoute()`
//      и dispatch'ит соответствующий internal channel.
//   3. Heart sidecar / code tools / vault picker — удалены в Phase 6.0.A.
//      Trash работает поверх ARK soft-delete (см. shim).

import { createApp, vaporInteropPlugin } from "vue";
import { createPinia } from "pinia";
import { PiniaColada } from "@pinia/colada";
import { installScrollFadeListener } from "@kosmos/visuals";

import {
  dispatchEdenCommand,
  installKeplerApiShim,
} from "./lib/kepler-api-shim";

// Shim should be installed BEFORE Vue app boot — App.vue / stores читают
// `window.api` в onMounted / initApp.
installKeplerApiShim();
installScrollFadeListener();

import App from "./App.vue";

import "./index.css";
import "./composables/useTheme";

// Pinia Colada — server-state layer над Pinia (queries / mutations / cache).
// Установлен 2026-05-19 в рамках Phase 14 (pilot). Сейчас не используется
// — еще нет ни одной useQuery/useMutation; store/eden.ts продолжает работать
// на обычной Pinia. Миграция отдельных queries — отдельный proof loop.
createApp(App)
  .use(createPinia())
  .use(PiniaColada)
  .use(vaporInteropPlugin)
  .mount("#root");

// ---------------------------------------------------------------------------
// Deep-link routing — static open-команды Kepler shell'а вызывают
// `openAsExtension("eden", "/today" | "/new")`. extension-host передаёт
// этот route через `kepler.navigation.initialRoute()` (первый запуск)
// и `onNavigate` (последующие invoke когда окно уже открыто).
// ---------------------------------------------------------------------------

interface KeplerNamespace {
  navigation?: {
    initialRoute: () => Promise<string | null>;
    onNavigate: (handler: (route: string) => void) => () => void;
  };
}

function handleRoute(route: string | null): void {
  if (!route) return;
  console.log("[eden-extension] route:", route);
  if (route === "/today") {
    dispatchEdenCommand("eden:cmd:note:open-today", null);
  } else if (route === "/new") {
    dispatchEdenCommand("eden:cmd:note:create", null);
  }
}

const kepler = (window as unknown as { kepler?: KeplerNamespace }).kepler;
if (kepler?.navigation) {
  // ВНИМАНИЕ: НЕ используем `initialRoute()` параллельно с `onNavigate()`.
  // Shell посылает `kepler:extension:navigation` event через did-finish-load
  // ДЛЯ ВСЕХ открытий, включая cold-launch (см. extension-host.ts ~line 1015).
  // Если читать ещё и initialRoute() — handleRoute срабатывает ДВА раза на
  // один открытый extension. Логи user'а 2026-05-19 показали именно это:
  // openTodayJournal вызывался дважды, создавая race между двумя set
  // currentEntry. onNavigate — единственный источник правды для route.
  kepler.navigation.onNavigate(handleRoute);
}
