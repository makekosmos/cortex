import type { Page } from "playwright";

type TestEntry = {
  id: string;
  title: string;
  content_json: string;
  created_at: number;
  updated_at: number;
  folder_id: string | null;
  type_id: string;
  header_layout: string | null;
  header_props_json: string;
  schema_version: number;
  deleted_at: number | null;
};

type TestVault = {
  rootPath: string;
  files: Array<{ path: string; relativePath: string; name: string; content: string }>;
  images: never[];
};

type TestApi = {
  openMarkdownVault: () => Promise<TestVault | null>;
  saveEntry: (entry: TestEntry) => Promise<{ ok: boolean; message?: string }>;
  listAllEntries: () => Promise<readonly TestEntry[]>;
};

type TestScope = typeof window & {
  api?: TestApi;
  __memoriaPartialImportWrites?: number;
  __memoriaPartialImportInjected?: boolean;
  __memoriaPartialImportIds?: string[];
  __memoriaPartialImportOpened?: boolean;
};

export async function installPartialImportFailure(page: Page, importRoot: string) {
  await page.evaluate(
    ({ importRoot }) => {
      // SAFETY: the test waits for Memoria to install window.api before this callback runs.
      const scope = window as TestScope;
      const api = scope.api;
      if (!api) throw new Error("Memoria window.api is unavailable");
      const originalSaveEntry = api.saveEntry;
      api.openMarkdownVault = async () => {
        scope.__memoriaPartialImportOpened = true;
        return {
          rootPath: importRoot,
          files: [
            {
              path: `${importRoot}\\first.md`,
              relativePath: "first.md",
              name: "first.md",
              content: "# Cortex partial import first\n\nFirst partial-import body",
            },
            {
              path: `${importRoot}\\second.md`,
              relativePath: "second.md",
              name: "second.md",
              content: "# Cortex partial import second\n\nSecond partial-import body",
            },
          ],
          images: [],
        };
      };
      api.saveEntry = async (entry) => {
        const result = await originalSaveEntry(entry);
        if (!scope.__memoriaPartialImportInjected) {
          scope.__memoriaPartialImportWrites = (scope.__memoriaPartialImportWrites ?? 0) + 1;
          (scope.__memoriaPartialImportIds ??= []).push(entry.id);
        }
        if (scope.__memoriaPartialImportWrites === 2 && !scope.__memoriaPartialImportInjected) {
          scope.__memoriaPartialImportInjected = true;
          return { ok: false, message: "E2E injected post-write failure" };
        }
        return result;
      };
    },
    { importRoot },
  );
}

export async function readPartialImportState(page: Page) {
  return page.evaluate(async () => {
    // SAFETY: installPartialImportFailure establishes this test-only window shape.
    const scope = window as TestScope;
    if (!scope.api) throw new Error("Memoria window.api is unavailable");
    return {
      opened: scope.__memoriaPartialImportOpened,
      injected: scope.__memoriaPartialImportInjected,
      writes: scope.__memoriaPartialImportWrites ?? 0,
      ids: scope.__memoriaPartialImportIds ?? [],
      entries: await scope.api.listAllEntries(),
      status: Array.from(document.querySelectorAll(".settings-row-desc-plain"))
        .map((element) => element.textContent?.trim())
        .filter(Boolean)
        .join(" "),
    };
  });
}
