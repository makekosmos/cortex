import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createApp, nextTick, vaporInteropPlugin, type App } from "vue";
import GeneralSettings from "../../src/components/settings/GeneralSettings.vue";
import { useEdenStore } from "../../src/store/eden";

let app: App<Element> | null = null;
let container: HTMLDivElement | null = null;

function installApi() {
  Object.defineProperty(window, "api", {
    configurable: true,
    writable: true,
    value: {
      getEdenVisibleObjectTypeIds: vi.fn(async () => []),
      setEdenVisibleObjectTypeIds: vi.fn(async (ids: string[]) => ids),
    },
  });
}

function mountGeneralSettings() {
  if (!container) throw new Error("container not initialized");
  const pinia = createPinia();
  setActivePinia(pinia);
  const eden = useEdenStore();
  eden.noteTypes = [];
  installApi();
  app = createApp(GeneralSettings);
  app.use(pinia).use(vaporInteropPlugin);
  app.mount(container);
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  document.body.innerHTML = "";
  container = document.createElement("div");
  document.body.appendChild(container);
});

afterEach(() => {
  app?.unmount();
  app = null;
  container?.remove();
  container = null;
});

describe("GeneralSettings", () => {
  test("does not expose TipTap migration or editor-switch controls", async () => {
    mountGeneralSettings();
    await nextTick();

    expect(document.querySelector('[data-testid="eden-tiptap-migrate-button"]')).toBeNull();
    expect(document.querySelector('[data-testid="eden-tiptap-migrate-confirm"]')).toBeNull();
    expect(document.querySelector('[data-testid="eden-tiptap-toggle"]')).toBeNull();
  });
});
