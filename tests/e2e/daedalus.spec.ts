import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir, REPO_ROOT } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

test("Daedalus: worktree, stream, approval, changes и reopen", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("daedalus-flow");
  const repo = path.join(dataDir, "source-repo");
  fs.mkdirSync(repo, { recursive: true });
  execFileSync("git", ["init"], { cwd: repo });
  execFileSync("git", ["config", "user.email", "daedalus@test.invalid"], { cwd: repo });
  execFileSync("git", ["config", "user.name", "Daedalus Test"], { cwd: repo });
  fs.writeFileSync(path.join(repo, "README.md"), "fixture\n");
  execFileSync("git", ["add", "README.md"], { cwd: repo });
  execFileSync("git", ["commit", "-m", "fixture"], { cwd: repo });

  const isolatedBackend = path.join(
    REPO_ROOT,
    ".tmp",
    "cargo-daedalus",
    "debug",
    "kepler-backend.exe",
  );
  const backendExe =
    process.env.KEPLER_BACKEND_EXE ??
    (fs.existsSync(isolatedBackend)
      ? isolatedBackend
      : path.join(REPO_ROOT, "target", "debug", "kepler-backend.exe"));

  const app = await launchKeplerWithDataDir(dataDir, {
    KEPLER_BACKEND_EXE: backendExe,
    DAEDALUS_FAKE_APP_SERVER_EXE: process.execPath,
    DAEDALUS_FAKE_APP_SERVER_SCRIPT: path.join(
      REPO_ROOT,
      "tests",
      "e2e",
      "fixtures",
      "daedalus-fake-app-server.mjs",
    ),
  });
  try {
    const launcher = await app.firstWindow();
    await waitForBackendReady(launcher);
    const windowPromise = app.waitForEvent("window");
    await launcher.evaluate(() => window.kepler.commands.invoke("daedalus:open"));
    let daedalus = await windowPromise;
    await daedalus.waitForLoadState("domcontentloaded");
    await daedalus.evaluate(
      (projectPath) => window.kepler.ark.request("agents.projects.add", { path: projectPath }),
      repo,
    );
    await daedalus.reload({ waitUntil: "domcontentloaded" });
    await daedalus.locator("select").first().selectOption({ index: 1 });
    await daedalus.locator("textarea").first().fill("Создай тестовый файл");
    await daedalus.getByRole("button", { name: "Запустить задачу" }).click();

    await expect(daedalus.locator(".session-view")).toBeVisible();
    await expect(daedalus.getByText("Нужно подтверждение.")).toBeVisible();
    await expect(daedalus.getByText("daedalus-e2e.txt", { exact: true })).toBeVisible();

    // Closing only the extension window must not stop the active backend session.
    await daedalus.close();
    const activeReopenPromise = app.waitForEvent("window");
    await launcher.evaluate(() => window.kepler.commands.invoke("daedalus:open"));
    daedalus = await activeReopenPromise;
    await daedalus.waitForLoadState("domcontentloaded");
    await expect(daedalus.getByRole("heading", { name: "Создай тестовый файл" })).toBeVisible();
    await expect(daedalus.getByRole("button", { name: "Разрешить" })).toBeVisible();

    await daedalus.getByRole("button", { name: "Новая задача" }).click();
    await daedalus.locator("select").first().selectOption({ index: 1 });
    await daedalus.locator("textarea").first().fill("Вторая параллельная задача");
    await daedalus.getByRole("button", { name: "Запустить задачу" }).click();
    await expect(
      daedalus.getByRole("heading", { name: "Вторая параллельная задача" }),
    ).toBeVisible();
    await expect(daedalus.locator(".session-row")).toHaveCount(2);
    await daedalus.getByTitle("Остановить").click();
    await expect(daedalus.locator(".status-pill", { hasText: "Остановлено" })).toBeVisible();

    await daedalus.locator(".session-row", { hasText: "Создай тестовый файл" }).click();
    await daedalus.getByRole("button", { name: "Разрешить" }).click();
    await expect(daedalus.locator(".status-pill", { hasText: "Готово" })).toBeVisible();

    await daedalus.getByRole("button", { name: "Новая задача" }).click();
    await daedalus.locator("select").first().selectOption({ index: 1 });
    await daedalus.locator("textarea").first().fill("Задай вопрос о варианте");
    await daedalus.getByRole("button", { name: "Автопроверка" }).click();
    await daedalus.getByRole("button", { name: "Запустить задачу" }).click();
    await expect(daedalus.getByRole("group", { name: /Как продолжить/ })).toBeVisible();
    await daedalus.getByLabel("Тщательно").check();
    await daedalus.getByRole("button", { name: "Ответить" }).click();
    await expect(daedalus.locator(".status-pill", { hasText: "Готово" })).toBeVisible();

    await daedalus.close();
    const reopenedPromise = app.waitForEvent("window");
    await launcher.evaluate(() => window.kepler.commands.invoke("daedalus:open"));
    daedalus = await reopenedPromise;
    await daedalus.waitForLoadState("domcontentloaded");
    await expect(daedalus.locator(".session-view")).toBeVisible();
    await expect(daedalus.getByRole("heading", { name: "Задай вопрос о варианте" })).toBeVisible();
  } finally {
    await app.close();
  }
});
