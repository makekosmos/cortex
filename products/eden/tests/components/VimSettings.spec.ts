import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createApp, vaporInteropPlugin } from "vue";

let app: ReturnType<typeof createApp> | null = null;
let container: HTMLDivElement | null = null;

async function mountVimSettings(): Promise<void> {
  const { default: VimSettings } = await import("../../src/components/settings/VimSettings.vue");

  if (!container) {
    throw new Error("Тестовый контейнер не инициализирован");
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
  test("показывает Vim-настройки, справочник команд и note для выключенного CM6", async () => {
    await mountVimSettings();

    await expect.poll(() => document.body.textContent ?? "").toContain("Vim");
    await expect
      .poll(() => document.body.textContent ?? "")
      .toContain("Режим команд для Markdown-редактора Eden");
    await expect
      .poll(() => document.querySelector('[data-testid="eden-vim-mode-toggle"]'))
      .not.toBeNull();
    await expect
      .poll(() => document.querySelector('[data-testid="vim-motion-group-modes"]'))
      .not.toBeNull();
    await expect
      .poll(() => document.querySelector('[data-testid="vim-motion-group-eden"]'))
      .not.toBeNull();
    await expect
      .poll(() => document.body.textContent ?? "")
      .toContain("Сначала включите Markdown-редактор в общих настройках.");
    await expect.poll(() => document.body.textContent ?? "").toContain(":w");
    await expect.poll(() => document.body.textContent ?? "").toContain(":q");
    await expect.poll(() => document.body.textContent ?? "").toContain(":wq");
    await expect.poll(() => document.body.textContent ?? "").toContain(":zen on");
    await expect.poll(() => document.body.textContent ?? "").toContain(":zen off");
  });

  test("скрывает предупреждение, когда CM6 включён в preferences singleton", async () => {
    await mountVimSettings();

    const { usePreferences } = await import("../../src/composables/usePreferences");
    const preferences = usePreferences();
    await preferences.ready();

    expect(document.body.textContent ?? "").toContain(
      "Сначала включите Markdown-редактор в общих настройках.",
    );

    preferences.setCmEditorEnabled(true);

    await expect.poll(() => document.querySelector(".vim-settings-note")).toBeNull();
    await expect
      .poll(() => document.querySelector('[data-testid="eden-vim-mode-toggle"]'))
      .not.toBeNull();
  });
});
