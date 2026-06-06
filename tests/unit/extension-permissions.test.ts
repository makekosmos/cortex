import { describe, expect, test } from "bun:test";
import {
  assertExtensionArkPermission,
  assertExtensionEventPermission,
  assertExtensionHostPermission,
  type ExtensionSource,
} from "../../platform/desktop/electron/extension-permissions";

const USER: ExtensionSource = "user";

describe("extension permissions", () => {
  test("trusted dev/bundled sources bypass manifest permission checks", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "eden",
        source: "dev",
        operation: "upsert_object",
        params: { object: { id: "n1", typeId: "note_obj" } },
      }),
    ).resolves.toBeUndefined();
  });

  test("user-installed extensions are denied by default", async () => {
    // Regression: 2026-06-04. User-installed extensions must not inherit the
    // full first-party ARK bridge just because preload exposes ark.request.
    await expect(
      assertExtensionArkPermission({
        extensionId: "third-party",
        source: USER,
        operation: "upsert_object",
        params: { object: { id: "n1", typeId: "note_obj" } },
      }),
    ).rejects.toThrow("objects.write:note_obj");
  });

  test("type-scoped object write permission allows only that object type", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "notes-tool",
        source: USER,
        manifestPermissions: ["objects.write:note_obj"],
        operation: "upsert_object",
        params: { object: { id: "n1", typeId: "note_obj" } },
      }),
    ).resolves.toBeUndefined();

    await expect(
      assertExtensionArkPermission({
        extensionId: "notes-tool",
        source: USER,
        manifestPermissions: ["objects.write:note_obj"],
        operation: "upsert_object",
        params: { object: { id: "t1", typeId: "task_obj" } },
      }),
    ).rejects.toThrow("objects.write:task_obj");
  });

  test("delete_object can use type-scoped permission after object type lookup", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "tasks-tool",
        source: USER,
        manifestPermissions: ["objects.write:task_obj"],
        operation: "delete_object",
        params: { id: "task-1" },
        resolveObjectType: async () => "task_obj",
      }),
    ).resolves.toBeUndefined();
  });

  test("commands.register requires permission and own namespace", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "tools",
        source: USER,
        manifestPermissions: ["commands.register"],
        operation: "commands.register",
        params: { commands: [{ id: "tools:open", title: "Open", category: "action" }] },
      }),
    ).resolves.toBeUndefined();

    await expect(
      assertExtensionArkPermission({
        extensionId: "tools",
        source: USER,
        manifestPermissions: ["commands.register"],
        operation: "commands.register",
        params: { commands: [{ id: "eden:open", title: "Open", category: "action" }] },
      }),
    ).rejects.toThrow("outside its namespace");
  });

  test("params.operation does not grant permission for a different operation", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["objects.read"],
        operation: "upsert_object",
        params: {
          operation: "list_objects",
          object: { id: "n1", typeId: "note_obj" },
        } as Record<string, unknown>,
      }),
    ).rejects.toThrow("objects.write:note_obj");
  });

  test("host-local and admin operations are denied without explicit capability", async () => {
    await expect(
      assertExtensionArkPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["objects.read"],
        operation: "file_index.open",
        params: { path: "C:/Users/example/report.pdf" },
      }),
    ).rejects.toThrow("hostIndex.write");

    await expect(
      assertExtensionArkPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["objects.read"],
        operation: "start_sync",
      }),
    ).rejects.toThrow("sync.admin");
  });

  test("event subscriptions require matching read/control permission", () => {
    expect(() =>
      assertExtensionEventPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["objects.read"],
        event: "entity_changed",
      }),
    ).not.toThrow();

    expect(() =>
      assertExtensionEventPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: [],
        event: "entity_changed",
      }),
    ).toThrow("objects.read");
  });

  test("userData read/write are separate host capabilities", () => {
    expect(() =>
      assertExtensionHostPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["userData.read"],
        capability: "userData.read",
      }),
    ).not.toThrow();

    expect(() =>
      assertExtensionHostPermission({
        extensionId: "reader",
        source: USER,
        manifestPermissions: ["userData.read"],
        capability: "userData.write",
      }),
    ).toThrow("userData.write");
  });
});
