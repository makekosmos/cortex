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

async function launchApp(
  homePath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-hevy-")),
  attempt = 0,
): Promise<LaunchedApp> {
  const testAppDataPath = path.join(homePath, "AppData", "Roaming");
  const testUserDataPath = path.join(testAppDataPath, "EdenTestUserData");
  const electronApp = await electron.launch({
    args: ["."],
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: "1",
      HOME: homePath,
      KOSMOS_TEST_APPDATA: testAppDataPath,
      KOSMOS_TEST_USER_DATA: testUserDataPath,
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
    if (attempt >= 2) throw error;
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
      if (typeof setVaultPath === "function") await setVaultPath(selectedVaultPath);
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

test.describe("Hevy Integration", () => {
  test.describe.configure({ mode: "serial" });

  test("should expose hevy API methods on window.api", async () => {
    test.setTimeout(30000);
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      const hevyMethods = await launch.window.evaluate(() => {
        const api = Reflect.get(window, "api") as Record<string, unknown>;
        return {
          hevyLogin: typeof api.hevyLogin,
          hevyLogout: typeof api.hevyLogout,
          hevyGetAuthStatus: typeof api.hevyGetAuthStatus,
          hevyGetAccount: typeof api.hevyGetAccount,
          hevyGetWorkoutCount: typeof api.hevyGetWorkoutCount,
          hevyFetchWorkouts: typeof api.hevyFetchWorkouts,
          hevyFetchAllWorkouts: typeof api.hevyFetchAllWorkouts,
          hevySyncWorkouts: typeof api.hevySyncWorkouts,
        };
      });
      expect(hevyMethods.hevyLogin).toBe("function");
      expect(hevyMethods.hevyLogout).toBe("function");
      expect(hevyMethods.hevyGetAuthStatus).toBe("function");
      expect(hevyMethods.hevyGetAccount).toBe("function");
      expect(hevyMethods.hevyGetWorkoutCount).toBe("function");
      expect(hevyMethods.hevyFetchWorkouts).toBe("function");
      expect(hevyMethods.hevyFetchAllWorkouts).toBe("function");
      expect(hevyMethods.hevySyncWorkouts).toBe("function");
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
    }
  });

  test("should return not logged in status initially", async () => {
    test.setTimeout(30000);
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      const authStatus = await launch.window.evaluate(async () => {
        const api = Reflect.get(window, "api") as {
          hevyGetAuthStatus: () => Promise<{ loggedIn: boolean; username: string | null }>;
        };
        return api.hevyGetAuthStatus();
      });
      expect(authStatus.loggedIn).toBe(false);
      expect(authStatus.username).toBeNull();
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
    }
  });

  test("should return error when fetching workouts without login", async () => {
    test.setTimeout(30000);
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      const result = await launch.window.evaluate(async () => {
        const api = Reflect.get(window, "api") as {
          hevyFetchWorkouts: (startIndex?: number) => Promise<{ ok: boolean; error?: string }>;
        };
        return api.hevyFetchWorkouts();
      });
      expect(result.ok).toBe(false);
      expect(result.error).toBe("Not logged in");
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
    }
  });

  test("should show connected apps settings page with Hevy card", async () => {
    test.setTimeout(60000);
    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-hevy-settings-"));
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      // Open settings
      await launch.window.locator('[data-testid="open-settings-btn"]').scrollIntoViewIfNeeded();
      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();

      // Navigate to Connected Apps tab
      await launch.window.locator('[data-testid="settings-nav-connected-apps"]').click();
      await expect(launch.window.locator('[data-testid="connected-apps-settings"]')).toBeVisible();

      // Hevy card should be visible
      await expect(launch.window.locator('[data-testid="hevy-card"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="hevy-status-disconnected"]')).toBeVisible();

      // Login button should be visible when not connected
      await expect(launch.window.locator('[data-testid="hevy-login-btn"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="hevy-login-btn"]')).toContainText(
        "Войти через Hevy",
      );

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should show editable system types in object types settings", async () => {
    test.setTimeout(60000);
    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-hevy-types-"));
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      // Open settings
      await launch.window.locator('[data-testid="open-settings-btn"]').scrollIntoViewIfNeeded();
      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();

      // Navigate to Object Types tab
      await launch.window.locator('[data-testid="settings-nav-object-types"]').click();
      await expect(launch.window.locator(".object-types-scene")).toBeVisible();

      // System types should be listed as built-in and openable in the editor
      await expect(launch.window.locator('[data-testid="system-type-note_obj"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="system-type-game_obj"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="system-type-system-type-workout"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="system-type-system-type-exercise"]')).toBeVisible();
      await launch.window.locator('[data-testid="system-type-game_obj"]').click();
      await expect(launch.window.locator('[data-testid="type-objects-view"]')).toBeVisible();
      await launch.window.locator(".type-objects-secondary-btn").click();
      await expect(launch.window.locator(".type-editor-chip")).toContainText("Контракт защищен кодом");

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should show diary space with today section and history", async () => {
    test.setTimeout(60000);
    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-hevy-diary-"));
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();
      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await launch.window.locator('[data-testid="settings-space-diary"]').click();
      await expect(launch.window.locator('[data-testid="space-view-diary"]')).toBeVisible();

      // Today section should be visible
      await expect(launch.window.locator('[data-testid="diary-today-section"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-today-section"]')).toContainText(
        "Сегодня",
      );

      // History section should be visible
      await expect(launch.window.locator('[data-testid="diary-history-section"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-history-section"]')).toContainText(
        "История тренировок",
      );

      // No workouts yet — should show empty message
      await expect(launch.window.locator('[data-testid="space-view-diary"]')).toContainText(
        "Подключите Hevy",
      );

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }
      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test("should show system types in all-properties space", async () => {
    test.setTimeout(60000);
    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-hevy-props-"));
    let launch: LaunchedApp | null = null;
    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.locator('[data-testid="open-settings-btn"]').click({ force: true });
      await expect(launch.window.locator(".settings-page")).toBeVisible();
      await launch.window.locator('[data-testid="settings-nav-spaces"]').click();
      await launch.window.locator('[data-testid="settings-space-all-properties"]').click();
      await expect(
        launch.window.locator('[data-testid="space-view-all-properties"]'),
      ).toBeVisible();

      // System types should appear in properties view
      await expect(
        launch.window.locator('[data-testid="space-view-all-properties"]'),
      ).toContainText("Тренировка");
      await expect(
        launch.window.locator('[data-testid="space-view-all-properties"]'),
      ).toContainText("Упражнение");

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
