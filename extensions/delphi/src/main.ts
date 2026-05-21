// Delphi Vue extension entry — Kepler shell host.
//
// Differences vs legacy `apps/delphi/ts/src/main.ts`:
//   1. Router history — всегда `createMemoryHistory()`. У extension'а нет
//      server / URL navigation, BrowserWindow грузит `file://.../dist/index.html`.
//   2. Command bus — регистрация Delphi-команд и подписка на `command_invoked`
//      через `window.kepler.ark`. Legacy main process больше не нужен.
//   3. Native main-process integrations (lan-sync:*, db:switchSpace и т.п.)
//      недоступны в extension renderer. `lib/electron-api-shim.ts` бридж'ит
//      legacy `window.electronAPI` поверх `window.kepler.ark.request`:
//      ARK операции (task_obj / time_entry_obj CRUD) транслируются, а P2P
//      sync / filesystem каналы становятся graceful no-op'ами. Это позволило
//      оставить компоненты (App.vue, ProjectPage, SpaceSetup, SpacesSettingsTab,
//      store/todos, services/space, services/storage) без массового rewrite'а.

// Shim должен быть установлен ДО mount'а Vue, поскольку App.vue читает
// `window.electronAPI` синхронно в setup-блоке.
// eslint-disable-next-line import/no-unassigned-import
import "./lib/electron-api-shim";

import { createApp } from "vue";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";

import App from "./App.vue";
import { useQuickEntry } from "./composables/useQuickEntry";
import { routes } from "./router";

// eslint-disable-next-line import/no-unassigned-import
import "./global.css";
// eslint-disable-next-line import/no-unassigned-import
import "./composables/useTheme";

const router = createRouter({
  history: createMemoryHistory(),
  routes,
});

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.mount("#root");

// Deep links через IPC: Kepler делает `openExtension("delphi", "/today")` —
// initialRoute() возвращает "/today" при cold start, onNavigate триггерится
// при повторном invoke'е уже открытого окна.
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

// ---------------------------------------------------------------------------
// Command bus — kepler.ark.subscribe("command_invoked", ...)
// ---------------------------------------------------------------------------
//
// Регистрируем commands через ARK runtime и слушаем dispatches от launcher'а.
// Команды живут на ARK side (commands.* operations), не в main process.

type CommandInvokedEvent = { id: string; params?: unknown };

const kepler = (
  window as unknown as {
    kepler?: {
      ark: {
        request: (op: string, params?: Record<string, unknown>) => Promise<unknown>;
        subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
      };
    };
  }
).kepler;

if (kepler) {
  void kepler.ark
    .request("commands.register", {
      commands: [
        {
          id: "delphi:task:create",
          title: "Создать задачу",
          subtitle: "Delphi",
          category: "action",
        },
        {
          id: "delphi:task:today",
          title: "Открыть сегодняшние задачи",
          subtitle: "Delphi",
          category: "action",
        },
      ],
    })
    .catch((err: unknown) => {
      // eslint-disable-next-line no-console
      console.warn("[delphi-extension] commands.register failed:", err);
    });

  const off = kepler.ark.subscribe("command_invoked", (payload: unknown) => {
    const event = payload as CommandInvokedEvent | null;
    if (!event || typeof event.id !== "string") return;
    if (event.id === "delphi:task:create") {
      // Static import: useQuickEntry также используется напрямую из App.vue /
      // QuickEntry.vue / *Page.vue — dynamic import здесь только триггерил
      // INEFFECTIVE_DYNAMIC_IMPORT warning, не давал реальной lazy-выгоды.
      useQuickEntry().show();
    } else if (event.id === "delphi:task:today") {
      void router.push("/today");
    }
  });

  window.addEventListener("beforeunload", () => {
    off();
  });
}
