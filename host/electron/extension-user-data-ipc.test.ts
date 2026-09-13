import { describe, expect, test } from "bun:test";
import type { WebContents } from "electron";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  renameSync,
  rmdirSync,
  rmSync,
  symlinkSync,
  unlinkSync,
} from "node:fs";
import { readFileSync } from "node:fs";
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
  test("fails closed when the app root itself redirects through a junction", async () => {
    const parent = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-parent-"));
    const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-outside-"));
    const root = path.join(parent, "app");
    try {
      const secret = path.join(outside, "secret.bin");
      appendFileSync(secret, Buffer.from("outside"));
      symlinkSync(outside, root, "junction");
      const store = createUserDataStore(root);
      const results = [
        await store.read("secret.bin"),
        await store.write("secret.bin", new Uint8Array([1])),
        await store.delete("secret.bin"),
        await store.stat("secret.bin"),
      ];
      for (const result of results) expect(result).toEqual({ ok: false, error: "io-error" });
      expect(readFileSync(secret, "utf8")).toBe("outside");
    } finally {
      if (existsSync(root)) unlinkSync(root);
      rmSync(parent, { recursive: true, force: true });
      rmSync(outside, { recursive: true, force: true });
    }
  });

  test("keeps every operation bound when a parent is replaced at the access boundary", async () => {
    for (const operation of ["read", "write", "delete", "stat"] as const) {
      const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
      const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-outside-"));
      try {
        const parent = path.join(root, "attachments");
        const moved = path.join(root, "attachments-original");
        const secret = path.join(outside, "secret.bin");
        const target = path.join(parent, "secret.bin");
        mkdirSync(parent);
        appendFileSync(target, Buffer.from("inside"));
        appendFileSync(secret, Buffer.from("outside"));
        const store = createUserDataStore(root, {
          beforeAccess: () => {
            renameSync(parent, moved);
            symlinkSync(outside, parent, "junction");
          },
        });

        const result =
          operation === "read"
            ? await store.read("attachments/secret.bin")
            : operation === "write"
              ? await store.write("attachments/secret.bin", new Uint8Array([1]))
              : operation === "delete"
                ? await store.delete("attachments/secret.bin")
                : await store.stat("attachments/secret.bin");
        expect(result).toEqual({ ok: false, error: "io-error" });
        expect(readFileSync(secret, "utf8")).toBe("outside");
      } finally {
        const parent = path.join(root, "attachments");
        if (existsSync(parent)) rmdirSync(parent);
        rmSync(root, { recursive: true, force: true });
        rmSync(outside, { recursive: true, force: true });
      }
    }
  });

  test("fails closed for a junction escaping the app root", async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-outside-"));
    try {
      const linked = path.join(root, "attachments");
      const secret = path.join(outside, "secret.bin");
      appendFileSync(secret, Buffer.from("outside"));
      symlinkSync(outside, linked, "junction");
      const store = createUserDataStore(root);

      for (const result of [
        await store.read("attachments/secret.bin"),
        await store.write("attachments/secret.bin", new Uint8Array([1])),
        await store.delete("attachments/secret.bin"),
        await store.stat("attachments/secret.bin"),
      ]) {
        expect(result).toEqual({ ok: false, error: "io-error" });
      }
      expect(readFileSync(secret, "utf8")).toBe("outside");
    } finally {
      rmSync(root, { recursive: true, force: true });
      rmSync(outside, { recursive: true, force: true });
    }
  });

  test("keeps binary reads bounded after the initial size check", () => {
    const source = readFileSync(new URL("./extension-user-data-ipc.ts", import.meta.url), "utf8");
    expect(source).not.toContain("readFile(file)");
    expect(source).toContain("file.read(buffer, 0, USER_DATA_MAX_BYTES + 1, 0)");
  });

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
