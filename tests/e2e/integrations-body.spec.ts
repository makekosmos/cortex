import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir, REPO_ROOT } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

const EVIDENCE_DIR = path.join(REPO_ROOT, ".agent", "tasks", "2026-07-17-integrations-body", "raw");

async function saveScreenshot(
  app: Awaited<ReturnType<typeof launchKeplerWithDataDir>>,
  hash: string,
  name: string,
) {
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  const screenshotBase64 = await app.evaluate(async ({ BrowserWindow }, expectedHash) => {
    const window = BrowserWindow.getAllWindows().find((candidate) =>
      candidate.webContents.getURL().includes(expectedHash),
    );
    if (!window) throw new Error(`window not found: ${expectedHash}`);
    const bounds = window.getBounds();
    window.setBounds({ ...bounds, width: bounds.width + 1 });
    await new Promise((resolve) => setTimeout(resolve, 100));
    window.setBounds(bounds);
    window.webContents.invalidate();
    await new Promise((resolve) => setTimeout(resolve, 250));
    return (await window.webContents.capturePage()).toPNG().toString("base64");
  }, hash);
  fs.writeFileSync(path.join(EVIDENCE_DIR, name), Buffer.from(screenshotBase64, "base64"));
}

test("dashboard integration cards open the common provider controls in a modal", async () => {
  const dataDir = freshDataDir("integrations-dashboard");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);

    const dashboardPromise = app.waitForEvent("window", { timeout: 10_000 });
    await launcher.evaluate(() => window.kepler.commands.invoke("dashboard:open"));
    const dashboard = await dashboardPromise;
    await dashboard.waitForLoadState("domcontentloaded");

    await dashboard.getByRole("button", { name: "Интеграции" }).click();
    await expect(dashboard.getByRole("heading", { name: "Интеграции" })).toBeVisible();
    await expect(dashboard.getByTestId("integration-card-hevy")).toBeVisible();
    await expect(dashboard.getByTestId("integration-card-toggl")).toBeVisible();
    await expect(dashboard.getByTestId("integration-card-hevy")).toContainText("Не подключено");
    await expect(dashboard.getByTestId("integration-card-toggl")).toContainText("Не подключено");
    await expect(dashboard.getByTestId("integration-card-hevy").locator("img")).toBeVisible();
    await expect(dashboard.getByTestId("integration-card-toggl").locator("img")).toBeVisible();
    await saveScreenshot(app, "#/dashboard/integrations", "integrations-dashboard.png");

    await dashboard.getByTestId("integration-card-hevy").click();
    const hevy = dashboard.locator(".provider-card").filter({ hasText: "Hevy" });

    await expect(hevy).toContainText("Тренировки и упражнения");
    await expect(hevy).toContainText("При первом импорте загружается вся доступная история");
    await expect(hevy.getByRole("button", { name: /Где получить/ })).toBeVisible();
    await expect(hevy.getByRole("button", { name: "Получить сейчас" })).toBeDisabled();
    await expect(
      hevy.getByRole("switch", { name: /Получать данные Hevy при входе в систему/ }),
    ).toBeChecked();
    await dashboard.getByRole("button", { name: "Закрыть" }).click();

    await dashboard.getByTestId("integration-card-toggl").click();
    const toggl = dashboard.locator(".provider-card").filter({ hasText: "Toggl Track" });
    await expect(toggl).toContainText("Записи учёта времени");
    await expect(toggl.getByRole("button", { name: /Где получить/ })).toBeVisible();
    await expect(toggl.getByRole("button", { name: "Получить сейчас" })).toBeDisabled();
    await toggl.getByLabel("Частота получения данных").selectOption("360");
    await expect(toggl.getByLabel("Частота получения данных")).toHaveValue("360");

    const raw = JSON.parse(fs.readFileSync(path.join(dataDir, "integrations.json"), "utf8")) as {
      toggl: { intervalMinutes: number };
    };
    expect(raw.toggl.intervalMinutes).toBe(360);
    expect(JSON.stringify(raw)).not.toContain("credential");

    await dashboard.screenshot({
      path: path.join(EVIDENCE_DIR, "integrations-modal-toggl.png"),
      animations: "disabled",
    });
  } finally {
    await app.close();
  }
});

test("body is built into the dashboard and visualizes Hevy strength and load", async () => {
  const dataDir = freshDataDir("body-dashboard");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);

    await launcher.evaluate(async () => {
      const now = new Date().toISOString();
      await window.kepler.ark.request("upsert_object_type", {
        object_type: {
          id: "workout_obj",
          name: "Тренировка",
          schemaJson: "{}",
          uiSchemaJson: "{}",
          createdAt: now,
          updatedAt: now,
          systemLocked: false,
        },
      });
      await window.kepler.ark.request("upsert_object", {
        object: {
          id: "hevy-workout:e2e",
          typeId: "workout_obj",
          title: "Жимовой день",
          contentJson: {},
          propsJson: {
            source: "hevy",
            externalId: "e2e",
            startedAt: now,
            endedAt: now,
            exercises: [
              {
                title: "Жим лёжа",
                primaryMuscleGroup: "chest",
                secondaryMuscleGroups: ["triceps"],
                equipmentCategory: "barbell",
                sets: [{ type: "normal", weight_kg: 100, reps: 5 }],
              },
            ],
          },
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        },
      });
      await window.kepler.ark.request("integrations.body_weight_set", { bodyWeightKg: 80 });
    });

    const dashboardPromise = app.waitForEvent("window", { timeout: 10_000 });
    await launcher.evaluate(() => window.kepler.commands.invoke("kosmos:body"));
    const dashboard = await dashboardPromise;
    await dashboard.waitForLoadState("domcontentloaded");

    await expect(dashboard.getByRole("heading", { name: "Тело" })).toBeVisible();
    await expect(dashboard.getByRole("button", { name: "Тело" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    const chest = dashboard.locator('polygon[aria-label^="Грудь:"]').first();
    await expect(chest).toHaveAttribute("aria-label", /5 из 5/);
    await chest.click();
    await expect(dashboard.locator(".body-details")).toContainText("Жим лёжа");
    await saveScreenshot(app, "#/dashboard/body", "body-development.png");

    await dashboard.getByRole("button", { name: "Нагрузка" }).click();
    await dashboard.getByRole("button", { name: "Месяц" }).click();
    await expect(chest).toHaveAttribute("aria-label", /500 кг тоннажа/);
    await chest.click();
    await expect(dashboard.locator(".body-details")).toContainText("500 кг");
    await saveScreenshot(app, "#/dashboard/body", "body-load-month.png");
  } finally {
    await app.close();
  }
});
