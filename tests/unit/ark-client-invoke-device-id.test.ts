import { describe, expect, test } from "bun:test";
import { ArkClient } from "../../core/ark/packages/ark/src/ark-client";

function makeClient(captured: Record<string, unknown>[]): ArkClient {
  return new ArkClient({
    spaceId: "kepler-default",
    deviceId: "device-under-test",
    deviceName: "Test Device",
    requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
      captured.push(req);
      return true as T;
    },
  });
}

describe("ArkClient.invokeOperation local write identity", () => {
  test("adds client device_id to raw local write operations", async () => {
    const captured: Record<string, unknown>[] = [];
    const client = makeClient(captured);

    await client.invokeOperation({
      operation: "upsert_object",
      object: {
        id: "obj-1",
        typeId: "task_obj",
        title: "Task",
        contentJson: {},
        propsJson: {},
        createdAt: "2026-06-04T00:00:00.000Z",
        updatedAt: "2026-06-04T00:00:00.000Z",
        deletedAt: null,
      },
    });

    expect(captured).toHaveLength(1);
    expect(captured[0]).toMatchObject({
      operation: "upsert_object",
      device_id: "device-under-test",
    });
  });

  test("overrides raw device_id on local writes", async () => {
    const captured: Record<string, unknown>[] = [];
    const client = makeClient(captured);

    await client.invokeOperation({
      operation: "delete_object",
      id: "obj-1",
      device_id: "spoofed-device",
    });

    expect(captured[0]).toMatchObject({
      operation: "delete_object",
      device_id: "device-under-test",
    });
  });

  test("does not add device_id to read-only raw operations", async () => {
    const captured: Record<string, unknown>[] = [];
    const client = makeClient(captured);

    await client.invokeOperation({ operation: "list_objects" });

    expect(captured[0]).toEqual({ operation: "list_objects" });
  });
});
