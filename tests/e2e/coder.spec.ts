import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { launchKepler, REPO_ROOT } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

test("coder keeps its data and overlays inside the full-width scroll surface", async () => {
  const app = await launchKepler({ slug: "coder-layout-cache" });
  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);
    await launcher.evaluate(async () => {
      const now = new Date().toISOString();
      await window.kepler.ark.request("upsert_object_type", {
        object_type: {
          id: "coding_submission_obj",
          name: "Отправка задачи",
          schemaJson: "{}",
          uiSchemaJson: "{}",
          createdAt: now,
          updatedAt: now,
          systemLocked: false,
        },
      });
      await window.kepler.ark.request("upsert_object_type", {
        object_type: {
          id: "coding_profile_obj",
          name: "Профиль программиста",
          schemaJson: "{}",
          uiSchemaJson: "{}",
          createdAt: now,
          updatedAt: now,
          systemLocked: false,
        },
      });
      await window.kepler.ark.request("upsert_object", {
        object: {
          id: "leetcode-submission:1",
          typeId: "coding_submission_obj",
          title: "Two Sum",
          contentJson: {},
          propsJson: {
            source: "leetcode",
            externalId: "1",
            problemSlug: "two-sum",
            problemNumber: "1",
            problemTitle: "Two Sum",
            status: "Accepted",
            accepted: true,
            language: "typescript",
            submittedAt: now,
          },
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        },
      });
      await window.kepler.ark.request("upsert_object", {
        object: {
          id: "codewars-completion:1",
          typeId: "coding_submission_obj",
          title: "Multiples of 3 and 5",
          contentJson: {},
          propsJson: {
            source: "codewars",
            username: "tester",
            externalId: "1",
            problemSlug: "multiples-of-3-and-5",
            problemTitle: "Multiples of 3 and 5",
            status: "Completed",
            accepted: true,
            language: "javascript",
            languages: ["javascript", "rust"],
            submittedAt: now,
            url: "https://www.codewars.com/kata/multiples-of-3-and-5",
          },
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        },
      });
      await window.kepler.ark.request("upsert_object", {
        object: {
          id: "codewars-profile:current",
          typeId: "coding_profile_obj",
          title: "Codewars — tester",
          contentJson: {},
          propsJson: {
            source: "codewars",
            username: "tester",
            honor: 544,
            leaderboardPosition: 134,
            rank: { name: "3 kyu", score: 2116 },
          },
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        },
      });
    });

    const dashboardPromise = app.waitForEvent("window", { timeout: 10_000 });
    await launcher.evaluate(() => window.kepler.commands.invoke("dashboard:open"));
    const dashboard = await dashboardPromise;
    await dashboard.waitForLoadState("domcontentloaded");
    await dashboard.evaluate(() => {
      const request = window.kepler.ark.request.bind(window.kepler.ark);
      let coderReads = 0;
      window.kepler.ark.request = ((operation: string, params?: Record<string, unknown>) => {
        if (operation === "list_objects_by_type") coderReads += 1;
        return request(operation, params);
      }) as typeof window.kepler.ark.request;
      Object.assign(window, { __coderReads: () => coderReads });
    });

    await dashboard.getByRole("button", { name: "Кодер" }).click();
    await expect(dashboard.locator(".submissions-panel")).toContainText("0001");
    const dimensions = await dashboard.evaluate(() => {
      const page = document.querySelector(".coder-page") as HTMLElement;
      const content = document.querySelector(".coder-content") as HTMLElement;
      const body = document.querySelector(".dashboard-body") as HTMLElement;
      const table = document.querySelector(".table-scroll") as HTMLElement;
      return {
        pageRight: page.getBoundingClientRect().right,
        bodyRight: body.getBoundingClientRect().right,
        pageClientWidth: page.clientWidth,
        pageScrollWidth: page.scrollWidth,
        contentWidth: content.getBoundingClientRect().width,
        tableClientWidth: table.clientWidth,
        tableScrollWidth: table.scrollWidth,
      };
    });
    expect(Math.abs(dimensions.pageRight - dimensions.bodyRight)).toBeLessThan(1);
    expect(dimensions.pageScrollWidth).toBe(dimensions.pageClientWidth);
    expect(dimensions.contentWidth).toBeLessThanOrEqual(700);
    expect(dimensions.tableScrollWidth).toBe(dimensions.tableClientWidth);
    await expect(dashboard.locator(".problem-number a")).toHaveAttribute(
      "href",
      "https://leetcode.com/problems/two-sum/",
    );

    const tabsX = (await dashboard.locator(".platform-tabs").boundingBox())?.x;
    await dashboard.getByRole("button", { name: "Codewars" }).click();
    await expect(dashboard.locator(".summary-grid")).toContainText("3 kyu");
    await expect(dashboard.locator(".summary-grid")).toContainText("544");
    await expect(dashboard.locator(".submissions-panel")).toContainText("Multiples of 3 and 5");
    await expect(dashboard.locator(".submissions-panel")).toContainText("javascript, rust");
    const screenshot = await app.evaluate(async ({ BrowserWindow }) => {
      const window = BrowserWindow.getAllWindows().find((candidate) =>
        candidate.webContents.getURL().includes("#/dashboard/coder"),
      );
      if (!window) throw new Error("coder window not found");
      const bounds = window.getBounds();
      window.setBounds({ ...bounds, width: bounds.width + 1 });
      await new Promise((resolve) => setTimeout(resolve, 100));
      window.setBounds(bounds);
      window.webContents.invalidate();
      await new Promise((resolve) => setTimeout(resolve, 250));
      return (await window.webContents.capturePage()).toPNG().toString("base64");
    });
    const screenshotPath = path.join(
      REPO_ROOT,
      ".agent",
      "tasks",
      "2026-07-18-codewars-integration",
      "raw",
      "coder-codewars.png",
    );
    fs.mkdirSync(path.dirname(screenshotPath), { recursive: true });
    fs.writeFileSync(screenshotPath, Buffer.from(screenshot, "base64"));
    expect((await dashboard.locator(".platform-tabs").boundingBox())?.x).toBe(tabsX);
    await dashboard.getByRole("button", { name: "LeetCode" }).click();

    await dashboard.locator(".activity-tooltip").last().hover();
    const tooltip = dashboard.locator("body > .kosmos-tooltip__content");
    await expect(tooltip).toBeVisible();
    const tooltipBox = await tooltip.boundingBox();
    expect((tooltipBox?.x ?? -1) >= 0).toBe(true);
    expect((tooltipBox?.x ?? 0) + (tooltipBox?.width ?? 0)).toBeLessThanOrEqual(
      await dashboard.evaluate(() => innerWidth),
    );

    const reads = await dashboard.evaluate(() =>
      (window as unknown as { __coderReads: () => number }).__coderReads(),
    );
    await dashboard.getByRole("button", { name: "Интеграции" }).click();
    await dashboard.getByRole("button", { name: "Кодер" }).click();
    await expect(dashboard.locator(".submissions-panel")).toContainText("0001");
    expect(
      await dashboard.evaluate(() =>
        (window as unknown as { __coderReads: () => number }).__coderReads(),
      ),
    ).toBe(reads);
  } finally {
    await app.close();
  }
});
