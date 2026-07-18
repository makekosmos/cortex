import { expect, test } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
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
