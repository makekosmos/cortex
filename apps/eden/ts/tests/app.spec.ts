import { test, expect, _electron as electron, type Page } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

type ElectronApp = Awaited<ReturnType<typeof electron.launch>>;

interface LaunchedApp {
  electronApp: ElectronApp;
  window: Page;
  pageErrors: string[];
  homePath: string;
}

interface EntryPayload {
  id: string;
  title: string;
  type_id: string | null;
  header_layout: string | null;
  header_props_json: string;
}

interface NoteTypePayload {
  id: string;
  name: string;
}

interface EdenPerfMetricSummary {
  count: number;
  p95Ms: number;
}

interface EdenPerfSummary {
  inputToNextPaint: EdenPerfMetricSummary;
  updateToNextPaint: EdenPerfMetricSummary;
  saveDuration: EdenPerfMetricSummary;
  longTaskCount: number;
}

async function launchApp(
  homePath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-home-")),
  attempt = 0,
): Promise<LaunchedApp> {
  const electronApp = await electron.launch({
    args: ["."],
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: "1",
      HOME: homePath,
      NODE_ENV: "development",
    },
  });

  try {
    const window = await electronApp.firstWindow({ timeout: 45000 });
    const pageErrors: string[] = [];

    window.on("console", (msg) => console.log(msg.text()));
    window.on("pageerror", (error) => {
      pageErrors.push(error.message);
      console.log("Page error:", error);
    });

    await window.waitForLoadState("domcontentloaded");
    await window.waitForSelector(".app-container", { timeout: 10000 });
    return { electronApp, window, pageErrors, homePath };
  } catch (error) {
    await electronApp.close();

    if (attempt >= 2) {
      throw error;
    }

    return launchApp(homePath, attempt + 1);
  }
}

async function ensureVault(launch: LaunchedApp, vaultPath: string): Promise<LaunchedApp> {
  const { window, electronApp, homePath } = launch;

  await window.waitForSelector(".app-container");

  await window.evaluate(async (selectedVaultPath: string) => {
    const api = Reflect.get(window, "api");
    if (api && typeof api === "object") {
      const setVaultPath = Reflect.get(api, "setVaultPath");
      if (typeof setVaultPath === "function") {
        await setVaultPath(selectedVaultPath);
      }
      const updateSidebarConfig = Reflect.get(api, "updateSidebarConfig");
      if (typeof updateSidebarConfig === "function") {
        await updateSidebarConfig({
          widget: { width: 320, collapsed: false },
        });
      }
    }
  }, vaultPath);

  await electronApp.close();
  const relaunched = await launchApp(homePath);
  await relaunched.window.waitForSelector(".widget-sidebar-wrapper", { state: "attached" });
  return relaunched;
}

async function createNote(
  launch: LaunchedApp,
  title: string,
  content: string,
): Promise<LaunchedApp> {
  const { window, electronApp, homePath } = launch;

  await window.evaluate(
    async ({ nextTitle, nextContent }: { nextTitle: string; nextContent: string }) => {
      const api = Reflect.get(window, "api") as
        | { saveEntry?: (entry: Entry) => Promise<SaveEntryResult> }
        | undefined;
      if (!api?.saveEntry) {
        return;
      }

      const now = Date.now();
      await api.saveEntry({
        id: crypto.randomUUID(),
        title: nextTitle,
        content_json: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: nextContent,
                },
              ],
            },
          ],
        }),
        created_at: now,
        updated_at: now,
        folder_id: null,
        type_id: null,
        header_layout: null,
        header_props_json: "{}",
        schema_version: 1,
      });
    },
    { nextTitle: title, nextContent: content },
  );

  await electronApp.close();
  const relaunched = await launchApp(homePath);
  await relaunched.window.waitForSelector(".widget-sidebar-wrapper", { state: "visible" });
  return relaunched;
}

async function createNoteType(page: Page, name: string) {
  await page.evaluate(async (noteTypeName: string) => {
    const api = Reflect.get(window, "api") as
      | { saveNoteType?: (noteType: NoteType) => Promise<SaveNoteTypeResult> }
      | undefined;
    if (!api?.saveNoteType) {
      return;
    }

    const now = Date.now();
    await api.saveNoteType({
      id: `note-type-${noteTypeName.toLowerCase().replace(/\s+/g, "-")}`,
      name: noteTypeName,
      slug: noteTypeName.toLowerCase().replace(/\s+/g, "-"),
      icon: "✦",
      color: "#7fb7ff",
      schema_json: JSON.stringify({
        fields: [
          {
            id: "title",
            label: "Title",
            kind: "text",
            required: false,
          },
        ],
      }),
      header_template_json: JSON.stringify({ kind: "default" }),
      created_at: now,
      updated_at: now,
    });
  }, name);
}

async function toggleZenMode(page: Page) {
  const shortcut = process.platform === "darwin" ? "Meta+Alt+Z" : "Control+Alt+Z";
  await page.keyboard.press(shortcut);
}

function getModShortcut(key: string) {
  return process.platform === "darwin" ? `Meta+${key}` : `Control+${key}`;
}

test.describe("Electron App", () => {
  test.describe.configure({ mode: "serial" });

  test("should open an existing note from the list without blank screen", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-playwright-"));
    const noteTitle = "Playwright reopen note";
    const noteContent = "Hello from Playwright reopen test";
    const secondTitle = "Second note for switching";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      launch = await createNote(launch, noteTitle, noteContent);
      launch = await createNote(launch, secondTitle, "Secondary body");

      await expect
        .poll(
          async () => {
            return launch?.window.evaluate(async () => {
              const api = Reflect.get(window, "api") as
                | { listEntries?: () => Promise<Array<{ title: string }>> }
                | undefined;
              const entries = api?.listEntries ? await api.listEntries() : [];
              return entries.map((entry) => entry.title);
            });
          },
          { timeout: 5000 },
        )
        .toContain(noteTitle);

      const savedNote = launch.window.locator(".widget-nav-item", { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();

      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();
      await expect(launch.window.locator(".title-input")).toHaveValue(noteTitle);
      await expect(launch.window.locator(".ProseMirror")).toContainText(noteContent);
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should support spaces, settings screen, and focus mode flows", async () => {
    test.setTimeout(120000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-playwright-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await createNoteType(launch.window, "Проект");
      launch = await createNote(launch, "Space alpha", "First note inside spaces.");
      launch = await createNote(launch, "Space beta", "Second note inside spaces.");

      if (process.platform === "darwin") {
        const sidebarChrome = await launch.window.evaluate(() => {
          const shell = document.querySelector(".widget-sidebar-wrapper .kepler-sidebar-shell");
          const toggle = document.querySelector('[data-testid="sidebar-toggle"]');
          if (!(shell instanceof HTMLElement) || !(toggle instanceof HTMLElement)) {
            return null;
          }

          const shellStyle = window.getComputedStyle(shell);
          const toggleRect = toggle.getBoundingClientRect();

          return {
            sidebarPaddingTop: Number.parseFloat(shellStyle.paddingTop),
            toggleTop: toggleRect.top,
            toggleLeft: toggleRect.left,
          };
        });
        expect(sidebarChrome).not.toBeNull();
        expect(sidebarChrome?.sidebarPaddingTop ?? 0).toBeLessThanOrEqual(12);
        expect(sidebarChrome?.toggleTop ?? 0).toBeLessThanOrEqual(24);
        expect(sidebarChrome?.toggleLeft ?? 0).toBeGreaterThanOrEqual(88);
      }

      await launch.window.locator('[data-testid="open-settings-btn"]').click();
      await expect(launch.window.locator(".settings-page")).toBeVisible();
      await expect(launch.window.locator('[data-testid="settings-nav-general"]')).toBeVisible();

      if (process.platform === "darwin") {
        const settingsChrome = await launch.window.evaluate(() => {
          const sidebarShell = document.querySelector('.settings-page .kepler-sidebar-shell');
          const header = document.querySelector('[data-testid="settings-content-header"]');
          if (!(sidebarShell instanceof HTMLElement) || !(header instanceof HTMLElement)) {
            return null;
          }
          const sidebarStyle = window.getComputedStyle(sidebarShell);
          const headerStyle = window.getComputedStyle(header);
          return {
            sidebarPaddingTop: Number.parseFloat(sidebarStyle.paddingTop),
            headerPaddingTop: Number.parseFloat(headerStyle.paddingTop),
            headerPaddingLeft: Number.parseFloat(headerStyle.paddingLeft),
          };
        });
        expect(settingsChrome).not.toBeNull();
        expect(settingsChrome?.sidebarPaddingTop ?? 0).toBeGreaterThanOrEqual(52);
        expect(settingsChrome?.headerPaddingTop ?? 0).toBeGreaterThanOrEqual(38);
        expect(settingsChrome?.headerPaddingLeft ?? 0).toBeGreaterThanOrEqual(92);
      }

      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await expect(launch.window.locator('[data-testid="spaces-settings-tab"]')).toBeVisible();

      await launch.window.locator('[data-testid="settings-space-all-objects"]').click();
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toContainText(
        "Space alpha",
      );
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toContainText(
        "Страница",
      );

      await launch.window.locator('[data-testid="open-settings-btn"]').click();
      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await launch.window.locator('[data-testid="settings-space-all-properties"]').click();
      await expect(
        launch.window.locator('[data-testid="space-view-all-properties"]'),
      ).toBeVisible();
      await expect(
        launch.window.locator('[data-testid="space-view-all-properties"]'),
      ).toContainText("Проект");

      await launch.window.locator('[data-testid="open-settings-btn"]').click();
      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await launch.window.locator('[data-testid="settings-space-all-notes"]').click();
      await expect(launch.window.locator('[data-testid="space-view-all-notes"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="space-view-all-notes"]')).toContainText(
        "Space beta",
      );

      await launch.window
        .locator('[data-testid="space-view-all-notes"] .space-table-row', { hasText: "Space beta" })
        .first()
        .click();
      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();

      await launch.window.locator('[data-testid="open-settings-btn"]').click();
      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await launch.window.locator('[data-testid="settings-space-diary"]').click();
      await expect(launch.window.locator('[data-testid="space-view-diary"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-today-section"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-history-section"]')).toBeVisible();
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should toggle zen mode, suspend shell UI, and collect typing metrics", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-zen-"));
    const noteTitle = "Zen mode note";
    const noteContent = "Base content for zen mode.";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, noteTitle, noteContent);

      const savedNote = launch.window.locator(".widget-nav-item", { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();

      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();

      await launch.window.evaluate(() => {
        window.__edenPerf?.reset();
      });

      const editor = launch.window.locator(".ProseMirror");
      await editor.click();
      await launch.window.keyboard.type(" Normal typing burst ".repeat(8), { delay: 4 });
      await launch.window.waitForTimeout(250);

      const normalSummary = await launch.window.evaluate(() => {
        return window.__edenPerf?.getSummary() ?? null;
      });

      expect(normalSummary).not.toBeNull();
      expect((normalSummary as EdenPerfSummary).inputToNextPaint.count).toBeGreaterThan(0);
      expect((normalSummary as EdenPerfSummary).updateToNextPaint.count).toBeGreaterThan(0);

      const searchBtn = launch.window.locator('[data-testid="widget-link-search"]');
      await searchBtn.click();
      await expect(launch.window.locator(".search-overlay")).toBeVisible();

      await toggleZenMode(launch.window);

      await expect(launch.window.locator(".search-overlay")).toHaveCount(0);
      await expect(launch.window.locator(".titlebar")).toHaveCount(0);
      await expect(launch.window.locator(".widget-sidebar-wrapper")).toHaveCount(0);
      await expect(launch.window.locator('[data-testid="zen-mode-exit"]')).toBeVisible();

      await launch.window.evaluate(() => {
        window.__edenPerf?.reset();
      });

      await editor.click();
      await launch.window.keyboard.type(" Zen typing burst ".repeat(12), { delay: 4 });
      await launch.window.waitForTimeout(250);

      const zenSummary = await launch.window.evaluate(() => {
        return window.__edenPerf?.getSummary() ?? null;
      });

      expect(zenSummary).not.toBeNull();
      expect((zenSummary as EdenPerfSummary).inputToNextPaint.count).toBeGreaterThan(0);
      expect((zenSummary as EdenPerfSummary).updateToNextPaint.count).toBeGreaterThan(0);

      const perfArtifactPath = path.join(
        process.cwd(),
        "../../../.agent/tasks/2026-04-14-eden-writer-performance/artifacts/typing-perf-playwright.json",
      );
      fs.mkdirSync(path.dirname(perfArtifactPath), { recursive: true });
      fs.writeFileSync(
        perfArtifactPath,
        JSON.stringify(
          {
            normalSummary,
            zenSummary,
          },
          null,
          2,
        ),
      );

      await launch.window.locator('[data-testid="zen-mode-exit"]').click();

      await expect(launch.window.locator(".widget-sidebar-wrapper")).toBeVisible();
      await expect(launch.window.locator('[data-testid="zen-mode-exit"]')).toHaveCount(0);

      await launch.window.waitForTimeout(1200);
      await expect(launch.window.locator(".ProseMirror")).toContainText("Zen typing burst");
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should render inline caret only inside the focused editor surface", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-inline-caret-"));
    const noteTitle = "Inline caret note";
    const noteContent = "Caret should appear in the editor flow.";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, noteTitle, noteContent);

      const savedNote = launch.window.locator(".widget-nav-item", { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();
      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();

      const editor = launch.window.locator(".ProseMirror");
      await editor.click();
      await expect(launch.window.locator(".pm-inline-caret-anchor")).toHaveCount(1);

      const titleInput = launch.window.locator(".title-input");
      await titleInput.click();
      await expect(launch.window.locator(".pm-inline-caret-anchor")).toHaveCount(0);
      await expect
        .poll(
          async () =>
            launch?.window.evaluate(() => {
              const caret = document.querySelector(".kepler-caret");
              return caret instanceof HTMLElement && !caret.classList.contains("kepler-caret--hidden");
            }),
          { timeout: 5000 },
        )
        .toBe(true);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should restore native selection mode while text is selected in title input", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-caret-selection-"));
    const noteTitle = "Selection handoff note";
    const noteContent = "Custom caret should not break text selection.";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, noteTitle, noteContent);

      const savedNote = launch.window.locator(".widget-nav-item", { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();

      const titleInput = launch.window.locator(".title-input");
      await expect(titleInput).toBeVisible();
      await titleInput.click();

      await expect
        .poll(
          async () =>
            launch?.window.evaluate(() => {
              const caret = document.querySelector(".kepler-caret");
              return caret instanceof HTMLElement && !caret.classList.contains("kepler-caret--hidden");
            }),
          { timeout: 5000 },
        )
        .toBe(true);

      const pointerSelectionMode = await titleInput.evaluate((node) => {
        if (!(node instanceof HTMLInputElement)) {
          return null;
        }

        node.focus();
        node.dispatchEvent(
          new PointerEvent("pointerdown", {
            bubbles: true,
            button: 0,
            buttons: 1,
            pointerId: 1,
          }),
        );

        return {
          caretColor: node.style.getPropertyValue("caret-color"),
        };
      });

      expect(pointerSelectionMode).toMatchObject({
        caretColor: "",
      });

      const expandedSelection = await titleInput.evaluate((node) => {
        if (!(node instanceof HTMLInputElement)) {
          return null;
        }

        node.focus();
        node.setSelectionRange(0, Math.min(5, node.value.length));
        document.dispatchEvent(new Event("selectionchange"));

        return {
          start: node.selectionStart,
          end: node.selectionEnd,
          caretColor: node.style.getPropertyValue("caret-color"),
        };
      });

      expect(expandedSelection).toMatchObject({
        start: 0,
        end: 5,
        caretColor: "",
      });

      await expect
        .poll(
          async () =>
            launch?.window.evaluate(() => {
              const caret = document.querySelector(".kepler-caret");
              return caret instanceof HTMLElement && caret.classList.contains("kepler-caret--hidden");
            }),
          { timeout: 5000 },
        )
        .toBe(true);

      const collapsedSelection = await titleInput.evaluate((node) => {
        if (!(node instanceof HTMLInputElement)) {
          return null;
        }

        const nextPos = node.value.length;
        node.focus();
        node.setSelectionRange(nextPos, nextPos);
        node.dispatchEvent(
          new PointerEvent("pointerup", {
            bubbles: true,
            button: 0,
            buttons: 0,
            pointerId: 1,
          }),
        );
        document.dispatchEvent(new Event("selectionchange"));

        return {
          start: node.selectionStart,
          end: node.selectionEnd,
        };
      });

      expect(collapsedSelection).toMatchObject({
        start: noteTitle.length,
        end: noteTitle.length,
      });

      await expect
        .poll(
          async () =>
            launch?.window.evaluate(() => {
              const caret = document.querySelector(".kepler-caret");
              const input = document.querySelector(".title-input");

              return {
                customVisible:
                  caret instanceof HTMLElement && !caret.classList.contains("kepler-caret--hidden"),
                caretColor:
                  input instanceof HTMLInputElement
                    ? input.style.getPropertyValue("caret-color")
                    : null,
              };
            }),
          { timeout: 5000 },
        )
        .toEqual({
          customVisible: true,
          caretColor: "transparent",
        });

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should tolerate composition-like input events without renderer errors", async () => {
    test.setTimeout(90000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-save-race-"));
    const noteTitle = "Save overlap note";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, noteTitle, "Initial body");

      const savedNote = launch.window.locator(".widget-nav-item", { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();

      const editor = launch.window.locator(".ProseMirror");
      await editor.click();
      await launch.window.keyboard.type(" Before overlap ", { delay: 4 });

      await editor.evaluate((element) => {
        element.dispatchEvent(new CompositionEvent("compositionstart", { data: "" }));
        element.dispatchEvent(new CompositionEvent("compositionupdate", { data: "こんにちは" }));
        element.focus();
        document.execCommand("insertText", false, "こんにちは");
      });
      await editor.evaluate((element) => {
        element.dispatchEvent(new CompositionEvent("compositionend", { data: "こんにちは" }));
      });

      await launch.window.waitForTimeout(250);

      await expect(editor).toBeVisible();
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should stay stable when navigating away during a delayed save", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-navigate-save-"));
    const firstTitle = "Navigate save A";
    const secondTitle = "Navigate save B";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, firstTitle, "Initial A");
      launch = await createNote(launch, secondTitle, "Initial B");

      const firstNote = launch.window.locator(".widget-nav-item", { hasText: firstTitle }).first();
      const secondNote = launch.window.locator(".widget-nav-item", { hasText: secondTitle }).first();
      await expect(firstNote).toBeVisible();
      await expect(secondNote).toBeVisible();

      await firstNote.click();
      const editor = launch.window.locator(".ProseMirror");
      await editor.click();

      await launch.window.evaluate(() => {
        const originalSave = window.api.saveEntry.bind(window.api);
        let delayed = false;

        window.api.saveEntry = async (entry: Entry) => {
          if (!delayed) {
            delayed = true;
            await new Promise((resolve) => window.setTimeout(resolve, 350));
          }

          return originalSave(entry);
        };
      });

      await launch.window.keyboard.type(" delayed navigation save", { delay: 4 });
      await launch.window.waitForTimeout(1250);
      await secondNote.click();

      await expect(launch.window.locator(".title-input")).toHaveValue(secondTitle);
      await launch.window.waitForTimeout(1200);

      await expect(launch.window.locator(".title-input")).toHaveValue(secondTitle);
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should recover from duplicate-title save failure after retrying with a unique title", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-save-retry-"));
    const existingTitle = "Duplicate target";
    const retryTitle = "Recovered unique title";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      launch = await createNote(launch, existingTitle, "First entry");
      launch = await createNote(launch, "Retry source", "Second entry");

      const retrySource = launch.window.locator(".widget-nav-item", { hasText: "Retry source" }).first();
      await expect(retrySource).toBeVisible();
      await retrySource.click();

      const titleInput = launch.window.locator(".title-input");
      await titleInput.fill(existingTitle);
      await launch.window.waitForTimeout(150);
      await launch.window.keyboard.press(getModShortcut("S"));
      await expect
        .poll(
          async () => {
            return launch?.window.locator(".save-conflict-badge").textContent();
          },
          { timeout: 10000 },
        )
        .toContain("Заметка с таким названием уже есть в этой папке");

      await titleInput.fill(retryTitle);
      await launch.window.keyboard.press(getModShortcut("S"));
      await launch.window.waitForTimeout(1200);

      const entries = await launch.window.evaluate(async () => {
        return window.api.listEntries();
      });

      expect(entries.some((entry) => entry.title === retryTitle)).toBe(true);
      await expect(launch.window.locator(".save-conflict-badge")).toHaveCount(0);
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should create a custom note type and render a typed note header", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-typed-note-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.evaluate(async () => {
        const api = Reflect.get(window, "api") as
          | { saveNoteType?: (noteType: NoteType) => Promise<SaveNoteTypeResult> }
          | undefined;
        if (!api?.saveNoteType) return;
        const now = Date.now();
        await api.saveNoteType({
          id: "note-type-chelovek",
          name: "Человек",
          slug: "chelovek",
          icon: "✦",
          color: "#7fb7ff",
          schema_json: JSON.stringify({
            fields: [
              { id: "name", label: "name", kind: "text", required: false },
              { id: "photo", label: "photo", kind: "image", required: false },
            ],
          }),
          header_template_json: JSON.stringify({
            kind: "centered_profile",
            primaryFieldIds: ["name"],
            secondaryFieldIds: [],
            imageFieldId: "photo",
          }),
          created_at: now,
          updated_at: now,
        });
      });

      // Reload to pick up the new type
      await launch.electronApp.close();
      launch = await launchApp(launch.homePath);
      await launch.window.waitForSelector(".widget-sidebar-wrapper", { state: "visible" });

      await launch.window.click('button[title="Новая заметка"]');
      await launch.window.locator(".title-input").fill("Ada Lovelace");
      await launch.window.locator('[data-testid="typed-note-trigger"]').click();
      await expect(launch.window.locator('[data-testid="typed-note-menu"]')).toBeVisible();
      await launch.window.locator('.note-type-menu-item:has-text("Человек")').click();
      await launch.window.locator('[data-testid="typed-note-field-name"]').fill("Ada Lovelace");
      await launch.window
        .locator('[data-testid="typed-note-field-photo"]')
        .fill(
          'data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><rect width="80" height="80" fill="%2390caf9"/></svg>',
        );
      await expect(launch.window.locator('[data-testid="typed-note-header"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="typed-note-primary"]')).toContainText(
        "Ada Lovelace",
      );
      await launch.window.locator(".ProseMirror").click();
      await launch.window.keyboard.insertText("First programmer note");
      await launch.window.waitForTimeout(1200);

      await expect
        .poll(
          async () => {
            return launch?.window.evaluate(async () => {
              const api = Reflect.get(window, "api") as
                | {
                    listEntries?: () => Promise<Array<EntryPayload>>;
                  }
                | undefined;
              const entries = api?.listEntries ? await api.listEntries() : [];
              return entries.some((currentEntry) => currentEntry.title === "Ada Lovelace");
            });
          },
          { timeout: 20000 },
        )
        .toBe(true);

      const typedEntry = await launch.window.evaluate(async () => {
        const api = Reflect.get(window, "api") as
          | {
              listEntries?: () => Promise<Array<EntryPayload>>;
              listNoteTypes?: () => Promise<Array<NoteTypePayload>>;
            }
          | undefined;
        const entries = api?.listEntries ? await api.listEntries() : [];
        const noteTypes = api?.listNoteTypes ? await api.listNoteTypes() : [];
        const entry = entries.find((currentEntry) => currentEntry.title === "Ada Lovelace") ?? null;
        const noteType = noteTypes.find((currentType) => currentType.name === "Человек") ?? null;
        return {
          entry,
          noteTypeId: noteType?.id ?? null,
        };
      });

      expect(typedEntry.entry).not.toBeNull();
      expect(typedEntry.noteTypeId).not.toBeNull();
      expect(typedEntry.entry?.type_id).toBe(typedEntry.noteTypeId);
      expect(typedEntry.entry?.header_layout).toBe("centered_profile");
      expect(typedEntry.entry?.header_props_json).toContain("Ada Lovelace");
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should use a single shared sidebar and support collapse/expand", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-sidebar-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.waitForSelector(".widget-sidebar-wrapper", { state: "visible" });
      await expect(launch.window.locator(".vault-sidebar-wrapper")).toHaveCount(0);
      await expect(launch.window.locator('[data-testid="vault-switcher-menu"]')).toHaveCount(0);

      const sidebarTheme = await launch.window.evaluate(() => {
        const root = document.documentElement;
        const sidebar = document.querySelector(".kepler-sidebar-content");
        if (!(sidebar instanceof HTMLElement)) return null;
        const style = window.getComputedStyle(sidebar);
        return {
          hasDarkClass: root.classList.contains("dark"),
          backgroundColor: style.backgroundColor,
        };
      });
      expect(sidebarTheme).not.toBeNull();
      expect(sidebarTheme?.hasDarkClass).toBe(true);
      expect(sidebarTheme?.backgroundColor).not.toBe("rgb(255, 255, 255)");

      const collapseBtn = launch.window.locator('[data-testid="sidebar-toggle"]');
      await expect(collapseBtn).toBeVisible();

      const sidebarWrapper = launch.window.locator(".widget-sidebar-wrapper");

      await collapseBtn.click();
      await launch.window.waitForTimeout(400);

      await expect(sidebarWrapper).toHaveClass(/collapsed/);

      const expandBtn = launch.window.locator('[data-testid="sidebar-toggle-external"]');
      await expect(expandBtn).toBeVisible();
      await expandBtn.click();
      await launch.window.waitForTimeout(400);

      await expect(sidebarWrapper).not.toHaveClass(/collapsed/);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should keep vault switching inside settings instead of the main sidebar", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-vault-settings-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);
      await launch.window.waitForSelector(".widget-sidebar-wrapper", { state: "visible" });
      await expect(launch.window.locator(".vault-sidebar-wrapper")).toHaveCount(0);
      await expect(launch.window.locator('[data-testid="vault-switcher-menu"]')).toHaveCount(0);

      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();
      await expect(launch.window.locator(".settings-tab")).toContainText(vaultPath);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should open search overlay via search button and search for notes", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-search-"));
    const noteTitle = "Unrelated title";
    const russianPartial = "токен";
    const englishPartial = "workf";
    const noteContent =
      "Русский токенизатор помогает находить совпадения и english workflows тоже.";

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      launch = await createNote(launch, noteTitle, noteContent);

      const searchBtn = launch.window.locator('[data-testid="widget-link-search"]');
      await expect(searchBtn).toBeVisible();

      await searchBtn.click();

      const searchOverlay = launch.window.locator(".search-overlay");
      await expect(searchOverlay).toBeVisible();

      const searchInput = launch.window.locator(".search-overlay-input");
      await expect(searchInput).toBeVisible();
      await expect(searchInput).toBeFocused();

      await searchInput.fill(russianPartial);
      await launch.window.waitForTimeout(500);

      const russianResult = launch.window
        .locator(".search-overlay-result-item")
        .filter({ hasText: noteTitle })
        .first();
      await expect(russianResult).toBeVisible();
      await expect(russianResult).toContainText("токенизатор");

      await searchInput.fill(englishPartial);
      await launch.window.waitForTimeout(500);

      const searchResult = launch.window
        .locator(".search-overlay-result-item")
        .filter({ hasText: noteTitle })
        .first();
      await expect(searchResult).toBeVisible();
      await expect(searchResult).toContainText("workflows");

      await searchResult.click();

      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();
      await expect(launch.window.locator(".title-input")).toHaveValue(noteTitle);

      const overlayAfterSelect = launch.window.locator(".search-overlay");
      await expect(overlayAfterSelect).toHaveCount(0);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should show slash command menu and apply heading formatting", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-slash-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.click('button[title="Новая заметка"]');

      const editor = launch.window.locator(".ProseMirror");
      await editor.click();
      await launch.window.keyboard.type("/");

      const slashMenu = launch.window.locator(".slash-commands");
      await expect(slashMenu).toBeVisible();
      await expect(slashMenu).toContainText("Заголовок 1");

      await launch.window.keyboard.press("Enter");
      await launch.window.keyboard.type("Slash heading");

      await expect(launch.window.locator(".ProseMirror h1")).toContainText("Slash heading");
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should keep the settings sidebar visible without a collapse affordance", async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-settings-sidebar-"));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();

      const settingsToggle = launch.window.locator(".settings-sidebar-shell [data-testid='sidebar-toggle']");
      await expect(settingsToggle).toHaveCount(0);
      await expect(launch.window.locator('[data-testid="settings-nav-back"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="settings-nav-general"]')).toBeVisible();
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });
});
