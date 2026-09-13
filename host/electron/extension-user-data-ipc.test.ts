import { describe, expect, test } from "bun:test";
import type { WebContents } from "electron";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  USER_DATA_MAX_BYTES,
  type ExtensionUserDataIpcOptions,
  type UserDataIpcHandler,
  createUserDataStore,
  registerExtensionUserDataIpc,
  validateUserDataKey,
} from "./extension-user-data-ipc";

describe("app-scoped binary user data", () => {
  test("accepts opaque path-safe keys and rejects filesystem paths", () => {
    expect(validateUserDataKey("attachments/task-1.bin")).toBe(true);
    for (const key of ["../escape", "attachments/../../escape", "/absolute", "C:\\escape", ""]) {
      expect(validateUserDataKey(key)).toBe(false);
    }
  });

  test("returns clear missing results and round-trips bytes with stat/delete", async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    try {
      const store = createUserDataStore(root);
      expect(await store.read("missing.bin")).toEqual({ ok: false, error: "not-found" });
      expect(await store.stat("missing.bin")).toEqual({ ok: false, error: "not-found" });

      expect(await store.write("attachments/task-1.bin", new Uint8Array([0, 1, 255]))).toEqual({
        ok: true,
        data: { sizeBytes: 3 },
      });
      expect(await store.read("attachments/task-1.bin")).toEqual({
        ok: true,
        data: new Uint8Array([0, 1, 255]),
      });
      expect(await store.stat("attachments/task-1.bin")).toEqual({
        ok: true,
        data: { sizeBytes: 3 },
      });
      expect(await store.delete("attachments/task-1.bin")).toEqual({ ok: true, data: true });
      expect(await store.delete("attachments/task-1.bin")).toEqual({
        ok: false,
        error: "not-found",
      });
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("rejects oversized writes without leaving temporary files", async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    try {
      const store = createUserDataStore(root);
      expect(await store.write("boundary.bin", new Uint8Array(USER_DATA_MAX_BYTES))).toEqual({
        ok: true,
        data: { sizeBytes: USER_DATA_MAX_BYTES },
      });
      const result = await store.write("large.bin", new Uint8Array(USER_DATA_MAX_BYTES + 1));
      expect(result).toEqual({ ok: false, error: "too-large" });
      expect(existsSync(path.join(root, "large.bin"))).toBe(false);
      expect(readdirSync(root)).toEqual(["boundary.bin"]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("authorizes every operation from the live app sender", async () => {
    const handlers = new Map<string, UserDataIpcHandler>();
    // SAFETY: these inert objects stand in for WebContents at the pure handler seam.
    const knownSender = {} as WebContents;
    // SAFETY: this inert object stands in for a different WebContents at the pure handler seam.
    const unknownSender = {} as WebContents;
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    try {
      registerExtensionUserDataIpc({
        handle: (channel, handler) => handlers.set(channel, handler),
        resolveAppForSender: (sender) =>
          sender === knownSender ? { appId: "agenda", permissions: ["filesystem.read"] } : null,
        userDataDirForApp: () => root,
      } satisfies ExtensionUserDataIpcOptions);
      const handler = handlers.get("host:user-data:binary")!;
      expect(
        await handler({ sender: knownSender }, { operation: "stat", key: "task.bin" }),
      ).toEqual({
        ok: false,
        error: "not-found",
      });
      await expect(
        handler({ sender: unknownSender }, { operation: "stat", key: "task.bin" }),
      ).rejects.toThrow("Unknown app sender");
      await expect(
        handler(
          { sender: knownSender },
          { operation: "write", key: "task.bin", bytes: new Uint8Array() },
        ),
      ).rejects.toThrow("User data permission denied");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
