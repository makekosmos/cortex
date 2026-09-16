import { describe, expect, test } from "../test-support/node-test.mjs";
import type { WebContents } from "electron";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  type ExtensionUserDataIpcOptions,
  type UserDataBackend,
  type UserDataIpcHandler,
  createEngineUserDataStores,
  registerExtensionUserDataIpc,
  validateUserDataKey,
} from "./extension-user-data-ipc";

function emptyBackend(): UserDataBackend {
  return {
    openRoot: () => Promise.resolve({ ok: true, data: { rootId: "root-1" } }),
    read: () => Promise.resolve({ ok: false, error: "not-found" }),
    write: () => Promise.resolve({ ok: true, data: { sizeBytes: 0 } }),
    stat: () => Promise.resolve({ ok: false, error: "not-found" }),
    delete: () => Promise.resolve({ ok: true, data: { deleted: true } }),
  };
}

describe("app-scoped binary user data IPC", () => {
  test("performs no pathname-based filesystem I/O in-process", () => {
    const source = readFileSync(new URL("./extension-user-data-ipc.ts", import.meta.url), "utf8");
    expect(source).not.toContain("node:fs/promises");
    for (const operation of ["realpath", "lstat", "writeFile", "rename(", "rm(", "open("])
      expect(source).not.toContain(operation);
  });

  test("accepts opaque path-safe keys and rejects filesystem paths", () => {
    expect(validateUserDataKey("attachments/task-1.bin")).toBe(true);
    for (const key of ["../escape", "attachments/../../escape", "/absolute", "C:\\escape", ""]) {
      expect(validateUserDataKey(key)).toBe(false);
    }
  });

  test("authorizes every operation from the live app sender", async () => {
    const handlers = new Map<string, UserDataIpcHandler>();
    // SAFETY: these inert objects stand in for WebContents at the pure handler seam.
    const knownSender = {} as WebContents;
    // SAFETY: this inert object stands in for a different WebContents at the pure handler seam.
    const unknownSender = {} as WebContents;
    const stores = createEngineUserDataStores(emptyBackend(), "root");
    const seen: string[] = [];
    registerExtensionUserDataIpc({
      handle: (channel, handler) => handlers.set(channel, handler),
      resolveAppForSender: (sender) =>
        sender === knownSender ? { appId: "agenda", permissions: ["filesystem.read"] } : null,
      userDataStoreForApp: (appId) => {
        seen.push(appId);
        return stores.forApp(appId);
      },
    } satisfies ExtensionUserDataIpcOptions);
    const handler = handlers.get("host:user-data:binary")!;
    expect(await handler({ sender: knownSender }, { operation: "stat", key: "task.bin" })).toEqual({
      ok: false,
      error: "not-found",
    });
    expect(seen).toEqual(["agenda"]);
    await expect(
      handler({ sender: unknownSender }, { operation: "stat", key: "task.bin" }),
    ).rejects.toThrow("Unknown app sender");
    await expect(
      handler(
        { sender: knownSender },
        { operation: "write", key: "task.bin", bytes: new Uint8Array() },
      ),
    ).rejects.toThrow("User data permission denied");
  });

  test("keeps sender permission checks ahead of store access", async () => {
    const handlers = new Map<string, UserDataIpcHandler>();
    // SAFETY: this inert object stands in for a WebContents at the pure handler seam.
    const sender = {} as WebContents;
    let storeRequests = 0;
    registerExtensionUserDataIpc({
      handle: (channel, handler) => handlers.set(channel, handler),
      resolveAppForSender: () => ({ appId: "agenda", permissions: ["filesystem.write"] }),
      userDataStoreForApp: () => {
        storeRequests += 1;
        return createEngineUserDataStores(emptyBackend(), "root").forApp("agenda");
      },
    });
    const handler = handlers.get("host:user-data:binary")!;
    await expect(handler({ sender }, { operation: "read", key: "task.bin" })).rejects.toThrow(
      "User data permission denied",
    );
    await expect(handler({ sender }, null)).rejects.toThrow("Invalid binary user data request");
    expect(storeRequests).toBe(0);
  });

  test("isolates stores per app id", async () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    try {
      const stores = createEngineUserDataStores(emptyBackend(), dir);
      expect(stores.forApp("app.one")).toBe(stores.forApp("app.one"));
      expect(stores.forApp("app.two")).not.toBe(stores.forApp("app.one"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
