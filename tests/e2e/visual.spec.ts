// Visual regression snapshots (Phase 6 bug-detection roadmap).
//
// Цель — дешёвая защита от CSS-регрессий. typecheck не ловит «layout
// поехал», unit-тесты не ловят «цвета перепутаны». Playwright
// `toHaveScreenshot` делает pixel-diff против baseline закоммиченного
// в репо.
//
// Платформа: Windows-only (snapshot создан на Windows, font hinting и
// rendering нестабилен cross-OS). `maxDiffPixels: 200` — небольшой
// запас на subpixel differences между Windows builds.
//
// Окна Kepler в e2e создаются `show: false` (headless mode) — screenshot
// всё равно работает через webContents.

import { test, expect, type Page } from "@playwright/test";
import { resolve } from "node:path";
import { SYSTEM_TYPE_BOOK } from "../../products/eden/src/lib/systemTypes";
import { LIVELIB_LIKE_BOOK_PAGE } from "../../products/eden/tests/fixtures/bookMetadataPages";
import { localImageUrl } from "../../platform/desktop/electron/local-image-protocol";
import { launchKepler } from "./helpers/launch";
import { openEden } from "./helpers/eden";
import { waitForBackendReady } from "./helpers/wait";

const SNAP_OPTS = { maxDiffPixels: 200 } as const;

async function captureElectronWindow(
  app: Awaited<ReturnType<typeof launchKepler>>,
  page: Page,
): Promise<Buffer> {
  const marker = `visual-${Date.now()}`;
  await page.evaluate((name) => {
    window.name = name;
  }, marker);
  const base64 = await app.evaluate(async ({ BrowserWindow }, name) => {
    for (const candidate of BrowserWindow.getAllWindows()) {
      const candidateName = await candidate.webContents.executeJavaScript("window.name");
      if (candidateName !== name) continue;
      const [width, height] = candidate.getSize();
      candidate.setSize(width + 1, height);
      await new Promise((resolveFrame) => setTimeout(resolveFrame, 50));
      candidate.setSize(width, height);
      candidate.webContents.invalidate();
      await new Promise((resolveFrame) => setTimeout(resolveFrame, 100));
      return (await candidate.capturePage()).toPNG().toString("base64");
    }
    throw new Error(`Electron window not found for ${name}`);
  }, marker);
  return Buffer.from(base64, "base64");
}

test.describe("visual regression", () => {
  test("launcher: initial state", async () => {
    const app = await launchKepler({ slug: "visual-launcher" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      // Стабилизируем перед скрином: дать Vue завершить mount.
      await launcher.waitForLoadState("networkidle").catch(() => {});
      await launcher.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await expect(launcher).toHaveScreenshot("launcher-initial.png", SNAP_OPTS);
    } finally {
      await app.close();
    }
  });

  test("eden: top navigation with everything mixed", async () => {
    test.setTimeout(60_000);
    const app = await launchKepler({ slug: "visual-eden-top-navigation" });
    try {
      const edenWindow = await openEden(app);
      await edenWindow.waitForFunction(
        () =>
          typeof (window as unknown as { api?: { saveEntry?: unknown; saveNoteType?: unknown } })
            .api?.saveEntry === "function" &&
          typeof (window as unknown as { api?: { saveNoteType?: unknown } }).api?.saveNoteType ===
            "function",
        null,
        { timeout: 5_000 },
      );

      const canReadLocalCoverPixels = await edenWindow.evaluate(
        (src) =>
          new Promise<boolean>((resolvePixels, reject) => {
            const image = new Image();
            image.crossOrigin = "anonymous";
            image.onload = () => {
              try {
                const canvas = document.createElement("canvas");
                canvas.width = 1;
                canvas.height = 1;
                const context = canvas.getContext("2d");
                if (!context) return resolvePixels(false);
                context.drawImage(image, 0, 0, 1, 1);
                resolvePixels(context.getImageData(0, 0, 1, 1).data[3]! > 0);
              } catch (error) {
                reject(error);
              }
            };
            image.onerror = () => reject(new Error("local image failed to load"));
            image.src = src;
          }),
        localImageUrl(resolve("products/eden/icon.png")),
      );
      expect(canReadLocalCoverPixels).toBe(true);

      const cover = (background: string, foreground: string, mark: string, ratio: string) =>
        `data:image/svg+xml,${encodeURIComponent(
          `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${ratio}"><rect width="100%" height="100%" fill="${background}"/><circle cx="50%" cy="36%" r="23%" fill="${foreground}"/><path d="M18 82 Q50 58 82 82 L82 100 L18 100Z" fill="${foreground}"/><text x="50%" y="38%" text-anchor="middle" dominant-baseline="middle" font-family="serif" font-size="16" fill="${background}">${mark}</text></svg>`,
        )}`;
      const content = (text: string) => JSON.stringify({ type: "markdown", version: 1, text });
      const baseTime = Date.UTC(2026, 6, 10, 12, 0, 0);
      const entries = [
        {
          id: "visual-book-ocean",
          title: "Море внутри",
          type_id: "book_obj",
          header_props_json: JSON.stringify({
            author: "Анна Север",
            cover_image: cover("midnightblue", "lightskyblue", "M", "100 150"),
          }),
          content_json: content("Заметки о первой главе."),
          updated_at: baseTime + 4_000,
        },
        {
          id: "visual-note-summer",
          title: "Идеи для летнего проекта",
          type_id: "note_obj",
          header_props_json: "{}",
          content_json: content("Собрать референсы и сделать первый прототип."),
          updated_at: baseTime + 3_000,
        },
        {
          id: "visual-book-sun",
          title: "Свет в окне",
          type_id: "book_obj",
          header_props_json: JSON.stringify({
            author: "Михаил Ладога",
            cover_image: cover("darkred", "peachpuff", "C", "100 142"),
          }),
          content_json: content("Любимые цитаты."),
          updated_at: baseTime + 2_000,
        },
        {
          id: "visual-book-fallback",
          title: "Невидимые города",
          type_id: "book_obj",
          header_props_json: JSON.stringify({ author: "Итало Кальвино" }),
          content_json: content("Без обложки — проверка типографической заглушки."),
          updated_at: baseTime + 1_000,
        },
      ].map((entry) => ({
        ...entry,
        created_at: baseTime,
        folder_id: null,
        header_layout: "default",
        schema_version: 1,
        deleted_at: null,
      }));

      await edenWindow.evaluate(
        async ({ bookType, entries }) => {
          const api = (
            window as unknown as {
              api: {
                saveNoteType: (noteType: unknown) => Promise<{ ok: boolean; reason?: string }>;
                saveEntry: (entry: unknown) => Promise<{ ok: boolean; reason?: string }>;
              };
            }
          ).api;
          const bookTypeResult = await api.saveNoteType(bookType);
          if (!bookTypeResult.ok) {
            throw new Error(`saveNoteType failed: ${JSON.stringify(bookTypeResult)}`);
          }

          for (const entry of entries) {
            const result = await api.saveEntry(entry);
            if (!result.ok) throw new Error(`saveEntry failed: ${JSON.stringify(result)}`);
          }
        },
        { bookType: SYSTEM_TYPE_BOOK, entries },
      );

      const closed = edenWindow.waitForEvent("close");
      await edenWindow.evaluate(() => window.api.close());
      await closed;

      const reopened = await openEden(app);
      await reopened.getByTestId("everything-view").waitFor({ state: "visible", timeout: 15_000 });
      await expect(reopened.locator('[data-testid^="everything-card-"]')).toHaveCount(4);
      const notePreview = reopened.getByTestId("everything-preview-visual-note-summer");
      await expect(notePreview).toBeVisible();
      await expect(notePreview).toContainText("Собрать референсы и сделать первый прототип.");
      await expect(reopened.getByTestId("eden-top-navigation")).toBeVisible();
      await expect(reopened.getByTestId("top-nav-everything")).toHaveAttribute(
        "aria-current",
        "page",
      );
      await expect(reopened.locator(".widget-sidebar-wrapper")).toHaveCount(0);
      const layoutBounds = await reopened.evaluate(() => {
        const rect = (selector: string) => {
          const bounds = document.querySelector(selector)?.getBoundingClientRect();
          return bounds ? { left: bounds.left, right: bounds.right, width: bounds.width } : null;
        };
        return {
          viewportWidth: document.documentElement.clientWidth,
          app: rect(".app-container"),
          chrome: rect(".kosmos-desktop-chrome-settings"),
          body: rect(".kosmos-desktop-chrome-settings__body"),
          main: rect(".app-main"),
        };
      });
      expect(layoutBounds.app?.width).toBe(layoutBounds.viewportWidth);
      expect(layoutBounds.chrome?.width).toBe(layoutBounds.viewportWidth);
      expect(layoutBounds.body?.width).toBe(layoutBounds.viewportWidth);
      expect(layoutBounds.main?.width).toBe(layoutBounds.viewportWidth);
      const navigationBounds = await reopened.getByTestId("eden-top-navigation").boundingBox();
      const indicatorBounds = await reopened.getByTestId("top-nav-indicator").boundingBox();
      const everythingBounds = await reopened.getByTestId("top-nav-everything").boundingBox();
      expect(
        Math.abs(
          (navigationBounds?.x ?? 0) +
            (navigationBounds?.width ?? 0) / 2 -
            layoutBounds.viewportWidth / 2,
        ),
      ).toBeLessThan(1);
      expect(navigationBounds?.height).toBeGreaterThanOrEqual(32);
      expect(
        Math.abs(
          (indicatorBounds?.x ?? 0) +
            (indicatorBounds?.width ?? 0) / 2 -
            ((everythingBounds?.x ?? 0) + (everythingBounds?.width ?? 0) / 2),
        ),
      ).toBeLessThan(1);
      await reopened.waitForFunction(
        () => {
          const view = document.querySelector('[data-testid="everything-view"]');
          const images = view ? Array.from(view.querySelectorAll("img")) : [];
          return (
            images.length === 2 && images.every((image) => image.complete && image.naturalWidth > 0)
          );
        },
        null,
        { timeout: 10_000 },
      );
      const card = reopened.getByTestId("everything-card-visual-book-ocean");
      expect(await card.evaluate((element) => getComputedStyle(element).cursor)).not.toBe(
        "pointer",
      );
      expect(
        await card
          .locator(".everything-item-visual")
          .evaluate((element) => getComputedStyle(element).transitionDuration),
      ).toBe("0.3s, 0.3s");
      await reopened.keyboard.press("Shift+G");
      const layoutGrid = reopened.getByTestId("eden-layout-grid");
      await expect(layoutGrid).toBeVisible();
      const fpsCounter = reopened.getByTestId("eden-fps-counter");
      await expect(fpsCounter).toBeVisible();
      await expect(fpsCounter).toHaveText(/^[1-9]\d* FPS$/);
      expect(
        await layoutGrid.evaluate((element) => {
          const style = getComputedStyle(element);
          return { backgroundSize: style.backgroundSize, pointerEvents: style.pointerEvents };
        }),
      ).toEqual({ backgroundSize: "8px 8px, 8px 8px", pointerEvents: "none" });
      const gridScreenshot = await captureElectronWindow(app, reopened);
      expect(gridScreenshot).toMatchSnapshot("eden-layout-grid.png", SNAP_OPTS);
      await reopened.keyboard.press("Shift+G");
      await expect(layoutGrid).toHaveCount(0);
      const coverEffect = await card
        .locator(".book-cover__layer")
        .evaluate((element) => getComputedStyle(element).backgroundImage);
      expect(coverEffect).toContain("rgb(25, 25, 112) 3px");
      expect(coverEffect).toContain("rgba(255, 255, 255, 0.5) 5px");
      await reopened.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await reopened.waitForTimeout(500);
      const screenshot = await captureElectronWindow(app, reopened);
      expect(screenshot).toMatchSnapshot("eden-top-navigation-mixed.png", SNAP_OPTS);

      await reopened.getByTestId("everything-card-visual-book-ocean").click();
      await reopened.locator(".ProseMirror").waitFor({ state: "visible", timeout: 15_000 });
      await expect(reopened.getByTestId("eden-top-navigation")).toHaveCount(0);
      const objectTitle = reopened.getByTestId("titlebar-page-title");
      await expect(objectTitle).toBeVisible();
      await expect(objectTitle).toContainText("Море внутри");
      const objectTitleBounds = await objectTitle.boundingBox();
      expect(
        Math.abs(
          (objectTitleBounds?.x ?? 0) +
            (objectTitleBounds?.width ?? 0) / 2 -
            layoutBounds.viewportWidth / 2,
        ),
      ).toBeLessThan(1);
      const bookHeader = reopened.getByTestId("typed-note-header");
      await expect(bookHeader).toHaveClass(/is-book/);
      const metadataButtonColors = await bookHeader
        .getByRole("button", { name: "Получить данные" })
        .evaluate((button) => {
          const probe = document.createElement("div");
          probe.style.background = "var(--foreground)";
          document.body.append(probe);
          const foreground = getComputedStyle(probe).backgroundColor;
          probe.remove();
          return { background: getComputedStyle(button).backgroundColor, foreground };
        });
      expect(metadataButtonColors.background).toBe(metadataButtonColors.foreground);
      const bookLayout = await bookHeader.evaluate((header) => {
        const host = header.closest(".tiptap-editor-host")!.getBoundingClientRect();
        const headerBounds = header.getBoundingClientRect();
        const visual = header
          .querySelector(".typed-object-header__visual")!
          .getBoundingClientRect();
        const fields = header
          .querySelector(".typed-object-header__secondary")!
          .getBoundingClientRect();
        const metadataButton = header
          .querySelector(".typed-object-header__metadata-button")!
          .getBoundingClientRect();
        return {
          topGap: headerBounds.top - host.top,
          visualRight: visual.right,
          visualTop: visual.top,
          visualBottom: visual.bottom,
          fieldsLeft: fields.left,
          titleBottom: header.querySelector(".typed-object-header__title")!.getBoundingClientRect()
            .bottom,
          fieldsBottom: fields.bottom,
          metadataButtonTop: metadataButton.top,
          metadataButtonBottom: metadataButton.bottom,
        };
      });
      expect(bookLayout.topGap).toBe(80);
      expect(bookLayout.visualRight).toBeLessThan(bookLayout.fieldsLeft);
      expect(bookLayout.titleBottom).toBeLessThanOrEqual(bookLayout.visualTop);
      expect(bookLayout.metadataButtonTop).toBeGreaterThanOrEqual(bookLayout.fieldsBottom);
      expect(Math.abs(bookLayout.metadataButtonBottom - bookLayout.visualBottom)).toBeLessThan(1);
      const objectCoverEffect = await bookHeader
        .locator(".book-cover__layer")
        .evaluate((element) => getComputedStyle(element).backgroundImage);
      expect(objectCoverEffect).toContain("rgb(25, 25, 112) 3px");
      expect(objectCoverEffect).toContain("rgba(255, 255, 255, 0.5) 5px");
      await reopened.waitForTimeout(500);
      const bookScreenshot = await reopened.screenshot();
      expect(bookScreenshot).toMatchSnapshot("eden-book-object-layout.png", SNAP_OPTS);
      await reopened.getByTestId("typed-note-field-language").getByRole("button").click();
      await expect(reopened.getByRole("option", { name: "Английский" })).toBeVisible();
      await reopened.keyboard.press("Escape");
      await reopened.setViewportSize({ width: 509, height: 755 });
      await reopened.waitForTimeout(100);
      const narrowBookWidths = await bookHeader.evaluate((header) => ({
        inner: header.querySelector(".typed-object-header__inner")!.getBoundingClientRect().width,
        visual: header.querySelector(".typed-object-header__visual")!.getBoundingClientRect().width,
      }));
      expect(Math.abs(narrowBookWidths.inner - narrowBookWidths.visual)).toBeLessThan(1);
      const narrowBookScreenshot = await captureElectronWindow(app, reopened);
      expect(narrowBookScreenshot).toMatchSnapshot("eden-book-object-narrow.png", SNAP_OPTS);
      await reopened.setViewportSize({ width: 1100, height: 750 });
      await reopened.getByTestId("tiptap-editor-host").evaluate((host) => {
        host.scrollTop = 0;
      });
      await reopened.waitForTimeout(100);
      await app.evaluate(({ ipcMain }, page) => {
        const channel = "kepler:extension:book-metadata:fetch-page";
        const isbnChannel = "kepler:extension:book-metadata:lookup-isbn";
        ipcMain.removeHandler(channel);
        ipcMain.removeHandler(isbnChannel);
        ipcMain.handle(channel, () => page);
        ipcMain.handle(isbnChannel, () => null);
      }, LIVELIB_LIKE_BOOK_PAGE);
      await bookHeader.getByRole("button", { name: "Получить данные" }).click();
      await reopened
        .getByRole("textbox", { name: "Ссылка или ISBN" })
        .fill(LIVELIB_LIKE_BOOK_PAGE.finalUrl);
      await reopened.getByRole("button", { name: "Найти" }).click();
      await reopened.getByTestId("book-metadata-preview").waitFor({ state: "visible" });
      await reopened.waitForTimeout(200);
      const metadataPreviewScreenshot = await reopened.screenshot();
      expect(metadataPreviewScreenshot).toMatchSnapshot(
        "eden-book-metadata-preview.png",
        SNAP_OPTS,
      );
      await reopened.getByRole("button", { name: "Применить" }).click();
      await expect(reopened.getByRole("dialog")).toBeHidden();
      await expect(reopened.getByTestId("typed-note-field-isbn")).toHaveValue("9780306406157");
      await expect(reopened.getByTestId("typed-note-field-language")).toContainText("Русский");
      await bookHeader.getByRole("button", { name: "Изменить обложку" }).click();
      await expect(reopened.getByRole("dialog")).toBeVisible();
      await reopened.waitForTimeout(500);
      const coverModalScreenshot = await reopened.screenshot();
      expect(coverModalScreenshot).toMatchSnapshot("eden-book-cover-modal.png", SNAP_OPTS);
      await reopened.locator('.typed-object-header__cover-file-input[type="file"]').setInputFiles({
        name: "local-book-cover.png",
        mimeType: "image/png",
        buffer: Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z4m8AAAAASUVORK5CYII=",
          "base64",
        ),
      });
      await expect(reopened.getByRole("dialog")).toBeHidden();
      await expect(bookHeader.locator(".book-cover__image")).toHaveAttribute(
        "src",
        /^kosmos-local-image:/,
      );
      const back = reopened.getByTestId("titlebar-history-back");
      await expect(back).toBeEnabled();
      await back.click();
      await reopened.getByTestId("everything-view").waitFor({ state: "visible" });

      await reopened.getByTestId("everything-card-visual-book-ocean").click();
      await reopened.locator(".ProseMirror").waitFor({ state: "visible", timeout: 15_000 });
      await reopened.evaluate(() => {
        const motionWindow = window as Window & { __edenPageTransitionStarted?: boolean };
        motionWindow.__edenPageTransitionStarted = false;
        const onTransitionRun = (event: TransitionEvent) => {
          if (event.propertyName !== "opacity") return;
          motionWindow.__edenPageTransitionStarted = true;
          document.removeEventListener("transitionrun", onTransitionRun);
        };
        document.addEventListener("transitionrun", onTransitionRun);
      });
      await expect(reopened.getByTestId("eden-top-navigation")).toHaveCount(0);
      await back.click();
      await reopened.getByTestId("everything-view").waitFor({ state: "visible" });
      await reopened.waitForFunction(
        () =>
          (window as Window & { __edenPageTransitionStarted?: boolean })
            .__edenPageTransitionStarted === true,
      );
      await reopened.getByTestId("top-nav-diary").click();
      expect(await reopened.getByTestId("top-nav-diary").getAttribute("aria-current")).toBe("page");
      await reopened.getByTestId("diary-view").waitFor({ state: "visible" });
      await expect(reopened.getByTestId("top-nav-diary")).toHaveAttribute("aria-current", "page");
      await reopened.waitForFunction(() => {
        const indicator = document
          .querySelector('[data-testid="top-nav-indicator"]')
          ?.getBoundingClientRect();
        const diary = document
          .querySelector('[data-testid="top-nav-diary"]')
          ?.getBoundingClientRect();
        return Boolean(
          indicator &&
          diary &&
          Math.abs(indicator.left + indicator.width / 2 - (diary.left + diary.width / 2)) < 1,
        );
      });
      await reopened.getByTestId("top-nav-everything").click();
      expect(await reopened.getByTestId("top-nav-everything").getAttribute("aria-current")).toBe(
        "page",
      );
      await reopened.getByTestId("everything-view").waitFor({ state: "visible" });
      await reopened.waitForFunction(() => {
        const indicator = document
          .querySelector('[data-testid="top-nav-indicator"]')
          ?.getBoundingClientRect();
        const everything = document
          .querySelector('[data-testid="top-nav-everything"]')
          ?.getBoundingClientRect();
        return Boolean(
          indicator &&
          everything &&
          Math.abs(
            indicator.left + indicator.width / 2 - (everything.left + everything.width / 2),
          ) < 1,
        );
      });

      await reopened.getByTestId("everything-card-visual-note-summer").click({ button: "right" });
      const contextMenu = reopened.getByRole("menu");
      await expect(contextMenu).toBeVisible();
      expect(await contextMenu.evaluate((element) => getComputedStyle(element).boxShadow)).toBe(
        "none",
      );
      const deleteItem = contextMenu.getByRole("menuitem", { name: "Удалить" });
      await expect(deleteItem.locator("svg")).toHaveCount(1);
      const contextMenuScreenshot = await captureElectronWindow(app, reopened);
      expect(contextMenuScreenshot).toMatchSnapshot("eden-everything-context-menu.png", SNAP_OPTS);
      await deleteItem.click();
      await expect(contextMenu).toHaveCount(0);
      await expect(reopened.getByTestId("everything-card-visual-note-summer")).toHaveCount(0);
      await expect(reopened.locator('[data-testid^="everything-card-"]')).toHaveCount(3);

      const cardsBeforeCreate = await reopened.locator('[data-testid^="everything-card-"]').count();
      await reopened.getByTestId("everything-add-card").click();
      await expect(reopened.getByTestId("eden-top-navigation")).toHaveCount(0);
      await expect(reopened.getByTestId("titlebar-page-title")).toBeVisible();
      await reopened.locator(".ProseMirror").waitFor({ state: "visible", timeout: 15_000 });
      await back.click();
      await reopened.getByTestId("everything-view").waitFor({ state: "visible" });
      await expect(reopened.locator('[data-testid^="everything-card-"]')).toHaveCount(
        cardsBeforeCreate + 1,
      );
    } finally {
      await app.close();
    }
  });

  test("launcher: stable after settle (regression baseline)", async () => {
    // Второй snapshot launcher'а после большего settle window — для уверенности
    // что между двумя независимыми launch'ами картинка детерминирована.
    const app = await launchKepler({ slug: "visual-launcher-settled" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      await launcher.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await launcher.waitForTimeout(1500);
      await expect(launcher).toHaveScreenshot("launcher-settled.png", {
        ...SNAP_OPTS,
        timeout: 10_000,
      });
    } finally {
      await app.close();
    }
  });
});
