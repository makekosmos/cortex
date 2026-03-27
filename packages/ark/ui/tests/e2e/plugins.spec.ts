import { test, expect } from "@playwright/test";

function primeAuthStorage() {
    localStorage.setItem("life_server_url", JSON.stringify("http://127.0.0.1:8010"));
    localStorage.setItem("life_api_key", JSON.stringify("test-secret"));
}

test.beforeEach(async ({ page }) => {
    await page.addInitScript(primeAuthStorage);
});

test("connects and renders basic stats", async ({ page }) => {
    await page.goto("/");

    // Header should show server url and total events.
    await expect(page.getByText("http://127.0.0.1:8010")).toBeVisible();
    await expect(page.getByText("событий")).toBeVisible();
});

test("plugins modal renders and allows toggling", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Плагины" }).click();
    await expect(page.getByText("Плагины")).toBeVisible();

    // List view
    const togglButton = page.getByRole("button", { name: "Toggl Track" });
    await expect(togglButton).toBeVisible();

    // Open detail view
    await togglButton.click();
    await expect(page.getByText("Импорт записей учёта времени из Toggl Track")).toBeVisible();

    // Toggle enabled
    await page.getByRole("button", { name: "Переключить плагин" }).click();
    // Go back to list and ensure badge appears.
    await page.getByRole("button", { name: "Назад к списку плагинов" }).click();
    await expect(page.getByText("АКТИВЕН")).toBeVisible();
});
