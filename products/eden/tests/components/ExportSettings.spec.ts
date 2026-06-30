import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createApp, nextTick, vaporInteropPlugin, type App } from "vue";
import ExportSettings from "../../src/components/settings/ExportSettings.vue";
import { writeEntryMarkdown } from "../../src/editor-cm/content";
import { SYSTEM_TYPE_NOTE } from "../../src/lib/systemTypes";
import { useEdenStore } from "../../src/store/eden";
import { markdownEntry } from "./cm-editor-test-helpers";

let app: App<Element> | null = null;
let container: HTMLDivElement | null = null;
let pinia: ReturnType<typeof createPinia> | null = null;

beforeEach(() => {
  document.body.innerHTML = "";
  container = document.createElement("div");
  document.body.appendChild(container);
  pinia = createPinia();
  setActivePinia(pinia);
});

afterEach(() => {
  app?.unmount();
  app = null;
  pinia = null;
  container?.remove();
  container = null;
});

describe("ExportSettings", () => {
  test("vault export reads full entries instead of summary bodies", async () => {
    if (!container) throw new Error("container not initialized");
    const summary = {
      ...markdownEntry(""),
      id: "note-1",
      title: "Note",
      content_loaded: false,
    };
    const full = {
      ...summary,
      content_json: JSON.stringify(writeEntryMarkdown("full body")),
      content_loaded: true,
    };
    const exportMarkdownVault = vi.fn(async () => ({
      outputDir: "D:/tmp/export",
      exportedCount: 1,
    }));

    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        listAllEntries: vi.fn(async () => [full]),
        listFolders: vi.fn(async () => []),
        exportMarkdownVault,
      },
    });

    const eden = useEdenStore();
    eden.entries = [summary];
    eden.noteTypes = [SYSTEM_TYPE_NOTE];

    app = createApp(ExportSettings);
    app.use(pinia!).use(vaporInteropPlugin).mount(container);
    await nextTick();

    const button = Array.from(container.querySelectorAll("button")).find((candidate) =>
      candidate.textContent?.includes("Экспорт vault"),
    );
    if (!button) throw new Error("export button not found");
    button.click();

    await expect.poll(() => exportMarkdownVault).toHaveBeenCalledTimes(1);
    const files = exportMarkdownVault.mock.calls[0]?.[0] as Array<{ content?: string }> | undefined;
    expect(files?.[0]?.content).toContain("full body");
  });
});
