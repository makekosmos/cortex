import { test, expect, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { freshDataDir, launchKeplerWithDataDir } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

function parseSyncVersionVector(rawVector: string | null): Record<string, string> {
  if (!rawVector) return {};
  const parsed: unknown = JSON.parse(rawVector);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
  return Object.fromEntries(
    Object.entries(parsed).filter(
      (entry): entry is [string, string] => typeof entry[1] === "string",
    ),
  );
}

function writeProbeExtension(dataDir: string, id: string, permissions: string[] | undefined): void {
  const dir = path.join(dataDir, "extensions", id);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "manifest.json"),
    JSON.stringify(
      {
        id,
        name: id,
        version: "1.0.0",
        keplerApiVersion: "^1.1.0",
        kind: "vue",
        entryHtml: "index.html",
        width: 360,
        height: 240,
        ...(permissions ? { permissions } : {}),
      },
      null,
      2,
    ),
  );
  fs.writeFileSync(
    path.join(dir, "index.html"),
    '<!doctype html><html><body><div id="ready">ready</div></body></html>',
    "utf8",
  );
}

async function openUserExtension(
  app: Awaited<ReturnType<typeof launchKeplerWithDataDir>>,
  id: string,
) {
  await app.evaluate(async ({ ipcMain }, extensionId) => {
    const handlers = (
      ipcMain as unknown as {
        _invokeHandlers: Map<string, (...a: unknown[]) => unknown>;
      }
    )._invokeHandlers;
    const handler = handlers?.get?.("kepler:extension:open");
    if (!handler) throw new Error("kepler:extension:open handler not registered");
    await handler({} as never, extensionId);
  }, id);
  const page = await app.waitForEvent("window", { timeout: 10_000 });
  await page.waitForLoadState("domcontentloaded");
  await page.waitForSelector("#ready", { timeout: 5_000 });
  return page;
}

async function seedTaskType(launcher: Page): Promise<void> {
  const now = new Date().toISOString();
  await launcher.evaluate(async (ts) => {
    await window.kepler.ark.request("upsert_object_type", {
      object_type: {
        id: "task_obj",
        name: "Task",
        schemaJson: "{}",
        uiSchemaJson: "{}",
        systemLocked: false,
        createdAt: ts,
        updatedAt: ts,
      },
    });
  }, now);
}

test.describe("extension runtime permissions", () => {
  test("user-installed extension without permissions cannot write ARK", async () => {
    const dataDir = freshDataDir("extension-permissions-deny");
    writeProbeExtension(dataDir, "perm-deny", undefined);

    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const ext = await openUserExtension(app, "perm-deny");
      const message = await ext.evaluate(async () => {
        try {
          await window.kepler.ark.request("upsert_object", {
            object: {
              id: "denied-task",
              typeId: "task_obj",
              title: "Denied",
              contentJson: {},
              propsJson: {},
              createdAt: new Date().toISOString(),
              updatedAt: new Date().toISOString(),
              deletedAt: null,
            },
          });
          return "allowed";
        } catch (err) {
          return err instanceof Error ? err.message : String(err);
        }
      });

      expect(message).toContain("not allowed");
      expect(message).toContain("objects.write:task_obj");
    } finally {
      await app.close();
    }
  });

  test("user-installed extension with scoped permission can write that object type", async () => {
    const dataDir = freshDataDir("extension-permissions-allow");
    writeProbeExtension(dataDir, "perm-allow", ["objects.read", "objects.write:task_obj"]);

    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      await seedTaskType(launcher);

      const ext = await openUserExtension(app, "perm-allow");
      const result = await ext.evaluate(async () => {
        const now = new Date().toISOString();
        await window.kepler.ark.request("upsert_object", {
          device_id: "spoofed-extension-device",
          object: {
            id: "allowed-task",
            typeId: "task_obj",
            title: "Allowed",
            contentJson: {},
            propsJson: {},
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          },
        });
        return window.kepler.ark.request<{ id: string; title: string }>("get_object", {
          id: "allowed-task",
        });
      });
      const rawVector = await launcher.evaluate(() =>
        window.kepler.ark.request<string | null>("get_sync_kv", {
          key: "lan_sync.version_vector",
        }),
      );
      const vector = parseSyncVersionVector(rawVector);

      expect(result?.id).toBe("allowed-task");
      expect(result?.title).toBe("Allowed");
      expect(vector["allowed-task"]).toMatch(/:kepler-shell-test-extension-permissions-allow$/);
    } finally {
      await app.close();
    }
  });

  test("params.operation override is rejected before forwarding", async () => {
    const dataDir = freshDataDir("extension-permissions-operation-override");
    writeProbeExtension(dataDir, "perm-override", ["objects.read"]);

    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const ext = await openUserExtension(app, "perm-override");
      const message = await ext.evaluate(async () => {
        try {
          await window.kepler.ark.request("list_objects", {
            operation: "upsert_object",
          } as Record<string, unknown>);
          return "allowed";
        } catch (err) {
          return err instanceof Error ? err.message : String(err);
        }
      });

      expect(message).toContain("params must not include operation");
    } finally {
      await app.close();
    }
  });
});
