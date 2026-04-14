import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { _electron as electron } from "/Users/kirill/Documents/projects/kepler/apps/eden/ts/node_modules/playwright/index.mjs";

const appDir = "/Users/kirill/Documents/projects/kepler/apps/eden/ts";
const homePath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-debug-home-"));
const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-debug-vault-"));

async function launch(home) {
  const electronApp = await electron.launch({
    args: ["."],
    cwd: appDir,
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: "1",
      HOME: home,
      NODE_ENV: "development",
    },
  });

  const window = await electronApp.firstWindow();
  await window.waitForLoadState("domcontentloaded");
  await window.waitForSelector(".app-container", { timeout: 10000 });

  return { electronApp, window };
}

let run = await launch(homePath);

await run.window.evaluate(async (selectedVaultPath) => {
  const api = Reflect.get(window, "api");
  await api.setVaultPath(selectedVaultPath);
  await api.updateSidebarConfig({
    vault: { width: 232, hidden: false },
    widget: { width: 320, hidden: false },
  });
}, vaultPath);

await run.electronApp.close();
run = await launch(homePath);

for (const [title, content] of [
  ["Space alpha", "First note inside spaces."],
  ["Space beta", "Second note inside spaces."],
]) {
  await run.window.evaluate(
    async ({ nextTitle, nextContent }) => {
      const api = Reflect.get(window, "api");
      const now = Date.now();
      await api.saveEntry({
        id: crypto.randomUUID(),
        title: nextTitle,
        content_json: JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph", content: [{ type: "text", text: nextContent }] }],
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
}

await run.electronApp.close();
run = await launch(homePath);

console.log(
  "entries before click",
  await run.window.evaluate(async () => {
    const api = Reflect.get(window, "api");
    const entries = await api.listEntries();
    return entries.map((entry) => entry.title);
  }),
);

await run.window.locator('[data-testid="widget-link-my-space"]').click();
await run.window.locator('[data-testid="widget-link-all-objects"]').click();

console.log(
  "entries after click",
  await run.window.evaluate(async () => {
    const api = Reflect.get(window, "api");
    const entries = await api.listEntries();
    return entries.map((entry) => entry.title);
  }),
);

console.log(
  "all objects text",
  await run.window.locator('[data-testid="space-view-all-objects"]').textContent(),
);

console.log(
  "recent buttons",
  await run.window.locator('[data-testid="main-sidebar"] .widget-nav-item').allTextContents(),
);

await run.electronApp.close();
fs.rmSync(homePath, { recursive: true, force: true });
fs.rmSync(vaultPath, { recursive: true, force: true });
