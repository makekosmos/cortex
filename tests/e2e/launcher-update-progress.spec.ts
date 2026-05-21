// Update progress bar в LauncherView (Фича C).
//
// `.update-tile-progress` рендерится только в state `downloading` —
// fill-элемент с inline `style.width = N%`. На `downloaded` / `available` он
// исчезает (computed updateBanner.progress === undefined).
//
// Symbol при downloading — Loader2 с класс `spin`.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { emitUpdateState } from "./helpers/update-state";
import { getLauncherWindow, waitForUpdateTile } from "./helpers/launcher";

test.describe("launcher update progress", () => {
  test("downloading state → fill-элемент с width=42%", async () => {
    const app = await launchKepler({ slug: "update-progress-42" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      await emitUpdateState(app, {
        kind: "downloading",
        version: "9.9.9",
        percent: 42,
      });

      await waitForUpdateTile(launcher, "downloading");

      const fill = launcher.locator(".update-tile-progress");
      await expect(fill).toBeVisible();
      const width = await fill.evaluate((el) => (el as HTMLElement).style.width);
      expect(width).toBe("42%");

      // Title содержит версию.
      await expect(launcher.locator(".update-tile .update-title")).toContainText(
        /9\.9\.9/,
      );
    } finally {
      await app.close();
    }
  });

  test("Loader2 крутится во время downloading (класс spin)", async () => {
    const app = await launchKepler({ slug: "update-progress-spin" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      await emitUpdateState(app, {
        kind: "downloading",
        version: "1.0.0",
        percent: 10,
      });

      await waitForUpdateTile(launcher, "downloading");

      // Loader2 SVG имеет класс .spin (см. :class="{ spin: updateBanner.spinning }").
      const spinning = launcher.locator(".update-tile .update-icon .spin");
      await expect(spinning).toBeVisible();
    } finally {
      await app.close();
    }
  });

  test("progress реактивно обновляется 0 → 50 → 100", async () => {
    const app = await launchKepler({ slug: "update-progress-reactive" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      const fill = launcher.locator(".update-tile-progress");

      await emitUpdateState(app, {
        kind: "downloading",
        version: "2.0.0",
        percent: 0,
      });
      await waitForUpdateTile(launcher, "downloading");
      await expect
        .poll(async () =>
          fill.evaluate((el) => (el as HTMLElement).style.width),
        )
        .toBe("0%");

      await emitUpdateState(app, {
        kind: "downloading",
        version: "2.0.0",
        percent: 50,
      });
      await expect
        .poll(async () =>
          fill.evaluate((el) => (el as HTMLElement).style.width),
        )
        .toBe("50%");

      await emitUpdateState(app, {
        kind: "downloading",
        version: "2.0.0",
        percent: 100,
      });
      await expect
        .poll(async () =>
          fill.evaluate((el) => (el as HTMLElement).style.width),
        )
        .toBe("100%");
    } finally {
      await app.close();
    }
  });

  test("downloaded state → fill исчезает, tile кликабельный", async () => {
    const app = await launchKepler({ slug: "update-progress-downloaded" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      // Сначала downloading.
      await emitUpdateState(app, {
        kind: "downloading",
        version: "3.0.0",
        percent: 80,
      });
      await waitForUpdateTile(launcher, "downloading");
      await expect(launcher.locator(".update-tile-progress")).toBeVisible();

      // Переход в downloaded.
      await emitUpdateState(app, { kind: "downloaded", version: "3.0.0" });
      await waitForUpdateTile(launcher, "downloaded");

      // Fill исчез — updateBanner.progress === undefined.
      await expect(launcher.locator(".update-tile-progress")).toHaveCount(0);

      // Tile НЕ имеет класса `disabled` (clickable: true в downloaded).
      const tile = launcher.locator(".update-tile").first();
      const hasDisabled = await tile.evaluate((el) =>
        el.classList.contains("disabled"),
      );
      expect(hasDisabled).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("available state → fill отсутствует, tile disabled", async () => {
    const app = await launchKepler({ slug: "update-progress-available" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      await emitUpdateState(app, { kind: "available", version: "4.0.0" });
      await waitForUpdateTile(launcher, "available");

      await expect(launcher.locator(".update-tile-progress")).toHaveCount(0);
      const tile = launcher.locator(".update-tile").first();
      const hasDisabled = await tile.evaluate((el) =>
        el.classList.contains("disabled"),
      );
      // updateBanner.clickable === false в available → tile должен быть disabled.
      expect(hasDisabled).toBe(true);
    } finally {
      await app.close();
    }
  });
});
