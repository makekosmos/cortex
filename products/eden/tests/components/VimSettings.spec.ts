import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createApp, vaporInteropPlugin } from "vue";

let app: ReturnType<typeof createApp> | null = null;
let container: HTMLDivElement | null = null;

async function mountVimSettings(): Promise<void> {
  const { default: VimSettings } = await import("../../src/components/settings/VimSettings.vue");

  if (!container) {
    throw new Error("Контейнер для монтирования не инициализирован");
  }

  app = createApp(VimSettings);
  app.use(vaporInteropPlugin);
  app.mount(container);
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  container = document.createElement("div");
  document.body.appendChild(container);
});

afterEach(() => {
  app?.unmount();
  app = null;
  container?.remove();
  container = null;
});

describe("VimSettings", () => {
  test("показывает Vim-настройки и справочник команд", async () => {
    await mountVimSettings();

    await expect.poll(() => document.body.textContent ?? "").toContain("Vim");
    await expect
      .poll(() => document.body.textContent ?? "")
      .toContain("Включает Vim-команды в CodeMirror-редакторе.");
    await expect
      .poll(() => document.querySelector('[data-testid="eden-vim-mode-toggle"]'))
      .not.toBeNull();
    await expect
      .poll(() => document.querySelector('[data-testid="vim-motion-group-modes"]'))
      .not.toBeNull();
    await expect
      .poll(() => document.querySelector('[data-testid="vim-motion-group-eden"]'))
      .not.toBeNull();
    await expect.poll(() => document.body.textContent ?? "").toContain(":w");
    await expect.poll(() => document.body.textContent ?? "").toContain(":q");
    await expect.poll(() => document.body.textContent ?? "").toContain(":wq");
    await expect.poll(() => document.body.textContent ?? "").toContain(":zen on");
    await expect.poll(() => document.body.textContent ?? "").toContain(":zen off");
  });

  test("не зависит от TipTap/CM6 флага preferences при отображении справочника", async () => {
    await mountVimSettings();

    const { usePreferences } = await import("../../src/composables/usePreferences");
    const preferences = usePreferences();
    await preferences.ready();

    preferences.setTiptapEditorEnabled(true);

    await expect
      .poll(() => document.body.textContent ?? "")
      .toContain("Включает Vim-команды в CodeMirror-редакторе.");
    await expect
      .poll(() => document.querySelector('[data-testid="vim-motion-group-modes"]'))
      .not.toBeNull();
  });
});
