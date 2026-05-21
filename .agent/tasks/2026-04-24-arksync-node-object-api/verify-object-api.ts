import { strict as assert } from "node:assert";

import { ArkClient, type ArkObjectRecord } from "../../../packages/arksync-node/src/index.ts";

const calls: Array<Record<string, unknown>> = [];
const injected = new ArkClient({
  spaceId: "space",
  deviceId: "device",
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req);
    switch (req.operation) {
      case "list_objects":
        return [] as T;
      case "get_object":
        return null as T;
      case "search_objects":
        return [{ file: "obj-1", line: 0, text: "match", entryId: "obj-1" }] as T;
      case "list_object_types":
        return [] as T;
      case "get_object_type":
        return null as T;
      case "list_object_links":
        return [] as T;
      default:
        return true as T;
    }
  },
});

const object: ArkObjectRecord = {
  id: "obj-1",
  typeId: "note_obj",
  title: "Note",
  contentJson: { blocks: [] },
  propsJson: {},
  createdAt: "2026-04-24T00:00:00.000Z",
  updatedAt: "2026-04-24T00:00:00.000Z",
  deletedAt: null,
};

await injected.objects.list();
await injected.objects.get("obj-1");
await injected.objects.upsert(object);
await injected.objects.delete("obj-1");
await injected.objects.search("note");
await injected.objectTypes.list();
await injected.objectTypes.get("note_obj");
await injected.objectTypes.upsert({
  id: "custom_obj",
  name: "Custom",
  schemaJson: "{}",
  uiSchemaJson: "{}",
  createdAt: "2026-04-24T00:00:00.000Z",
  updatedAt: "2026-04-24T00:00:00.000Z",
  systemLocked: false,
});
await injected.objectTypes.delete("custom_obj");
await injected.links.list();
await injected.links.upsert({
  id: "link-1",
  sourceObjectId: "obj-1",
  targetObjectId: "obj-2",
  linkType: "related",
  createdAt: "2026-04-24T00:00:00.000Z",
});
await injected.links.delete("link-1");

assert.deepEqual(
  calls.map((call) => call.operation),
  [
    "list_objects",
    "get_object",
    "upsert_object",
    "delete_object",
    "search_objects",
    "list_object_types",
    "get_object_type",
    "upsert_object_type",
    "delete_object_type",
    "list_object_links",
    "upsert_object_link",
    "delete_object_link",
  ],
);
assert.equal("id" in calls[0], false, "injected object API must not force request ids");
assert.deepEqual(calls[2].object, object);
assert.equal(calls[7].object_type && typeof calls[7].object_type, "object");
assert.equal(calls[10].object_link && typeof calls[10].object_link, "object");

const selfManagedCalls: Array<Record<string, unknown>> = [];
const selfManaged = new ArkClient({
  spaceId: "space",
  deviceId: "device",
  dbPath:
    "D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-24-arksync-node-object-api/self-managed/ark.db",
  sidecarPath: "unused",
}) as ArkClient & { requestViaChild: (req: Record<string, unknown>) => Promise<unknown> };
selfManaged.requestViaChild = async (req: Record<string, unknown>) => {
  selfManagedCalls.push(req);
  return req.operation === "list_objects" ? [] : true;
};

await selfManaged.objects.list();
assert.deepEqual(
  selfManagedCalls.map((call) => call.operation),
  ["init", "list_objects"],
  "self-managed object API should initialize before first object request",
);

console.log("verify-object-api PASS");
