// Eden Vue extension entry — Kepler shell host.
//
// Архитектурные отличия от standalone `apps/eden/ts/src/main.ts`:
//   1. Нет собственного Electron main процесса — `window.api` эмулируется
//      shim'ом (`./lib/kepler-api-shim`), который роутит ARK-операции через
//      `window.kepler.ark.request(operation, params)`.
//   2. Command bus — регистрируем `eden:note:create` и `eden:note:search`
//      через `kepler.ark.request("commands.register", ...)` и слушаем
//      `command_invoked` через `kepler.ark.subscribe(...)`.
//   3. Heart sidecar / code tools / vault picker — удалены в Phase 6.0.A.
//      Trash работает поверх ARK soft-delete (см. shim).

import { createApp, vaporInteropPlugin } from "vue";
import { createPinia } from "pinia";

import {
  dispatchEdenCommand,
  installKeplerApiShim,
} from "./lib/kepler-api-shim";

// Shim should be installed BEFORE Vue app boot — App.vue / stores читают
// `window.api` в onMounted / initApp.
installKeplerApiShim();

import App from "./App.vue";

import "./index.css";
import "./composables/useTheme";

createApp(App).use(createPinia()).use(vaporInteropPlugin).mount("#root");

// ---------------------------------------------------------------------------
// Command bus integration
// ---------------------------------------------------------------------------

interface CommandInvokedEvent {
  id: string;
  params?: unknown;
}

interface KeplerNamespace {
  ark: {
    request: <T = unknown>(
      operation: string,
      params?: Record<string, unknown>,
    ) => Promise<T>;
    subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
  };
}

const kepler = (window as unknown as { kepler?: KeplerNamespace }).kepler;

if (kepler) {
  void kepler.ark
    .request("commands.register", {
      commands: [
        {
          id: "eden:note:create",
          title: "Создать заметку",
          subtitle: "Eden",
          category: "action",
        },
        {
          id: "eden:note:search",
          title: "Поиск по заметкам",
          subtitle: "Eden",
          category: "action",
        },
      ],
    })
    .catch((e: unknown) => {
      console.warn("[eden-extension] commands.register failed:", e);
    });

  const off = kepler.ark.subscribe("command_invoked", (payload: unknown) => {
    const event = payload as CommandInvokedEvent | null;
    if (!event || typeof event.id !== "string") return;
    if (!event.id.startsWith("eden:")) return;

    if (event.id === "eden:note:create") {
      dispatchEdenCommand("eden:cmd:note:create", event.params ?? null);
      return;
    }
    if (event.id === "eden:note:search") {
      dispatchEdenCommand("eden:cmd:note:search", event.params ?? null);
    }
  });

  window.addEventListener("beforeunload", () => {
    try {
      off();
    } catch {
      // ignore
    }
  });
}
