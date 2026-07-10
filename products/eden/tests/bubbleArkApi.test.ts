import { describe, expect, test } from "bun:test";
import { createBubbleApi } from "../src/lib/kepler-bubble-api";

type ArkObject = {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
};

function reverseJsonKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(reverseJsonKeys);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .reverse()
      .map(([key, nested]) => [key, reverseJsonKeys(nested)]),
  );
}

function fakeArk(options: { reorderJsonOnRead?: boolean } = {}) {
  const objects = new Map<string, ArkObject>();
  const links = new Map<string, Record<string, unknown>>();
  const calls: string[] = [];
  const request = async (operation: string, params: Record<string, unknown> = {}) => {
    calls.push(operation);
    switch (operation) {
      case "list_objects_by_type":
        return [...objects.values()].filter((object) => object.typeId === params.type_id);
      case "list_object_links":
        return [...links.values()];
      case "get_object": {
        const object = objects.get(String(params.id)) ?? null;
        return object && options.reorderJsonOnRead
          ? { ...object, contentJson: reverseJsonKeys(object.contentJson) }
          : object;
      }
      case "upsert_object": {
        const object = structuredClone(params.object) as ArkObject;
        objects.set(object.id, object);
        return true;
      }
      case "delete_object": {
        const object = objects.get(String(params.id));
        if (object) object.deletedAt = new Date().toISOString();
        return true;
      }
      case "upsert_object_link": {
        const link = structuredClone(params.object_link) as Record<string, unknown>;
        links.set(String(link.id), link);
        return true;
      }
      case "delete_object_link":
        links.delete(String(params.id));
        return true;
      default:
        throw new Error(`unexpected ${operation}`);
    }
  };
  return { api: createBubbleApi(request), objects, links, calls };
}

describe("ARK bubble API", () => {
  test("persists the canonical contract and excludes unmarked journals", async () => {
    const ark = fakeArk();
    ark.objects.set("ordinary", {
      id: "ordinary",
      typeId: "system-type-journal",
      title: "ordinary",
      contentJson: {},
      propsJson: {},
      createdAt: "2026-07-01T00:00:00.000Z",
      updatedAt: "2026-07-01T00:00:00.000Z",
      deletedAt: null,
    });

    const id = await ark.api.createBubble("Текст #Tag #tag");
    const object = ark.objects.get(id)!;
    expect(object).toMatchObject({
      id,
      typeId: "system-type-journal",
      propsJson: { entry_kind: "bubble", bubble_kind: "plain", tags: ["tag"] },
      deletedAt: null,
    });
    expect(await ark.api.listBubbles()).toHaveLength(1);
  });

  test("updates in place, preserves occurrence and unrelated props", async () => {
    const ark = fakeArk();
    const id = await ark.api.createBubble("До");
    const before = ark.objects.get(id)!;
    before.propsJson.unrelated = 42;
    const createdAt = before.createdAt;

    await ark.api.updateBubble(id, { input: "После #новое", kind: "idea" });
    const after = ark.objects.get(id)!;
    expect(after.createdAt).toBe(createdAt);
    expect(after.updatedAt >= before.updatedAt).toBe(true);
    expect(after.propsJson).toMatchObject({
      entry_kind: "bubble",
      bubble_kind: "idea",
      tags: ["новое"],
      unrelated: 42,
    });
  });

  test("reconstructs replies and preserves children when deleting a root", async () => {
    const ark = fakeArk();
    const root = await ark.api.createBubble("Корень");
    const child = await ark.api.createBubble("Ответ", "plain", root);
    const sibling = await ark.api.createBubble("Второй", "plain", root);

    expect((await ark.api.listBubbles()).find((bubble) => bubble.id === child)?.parentId).toBe(
      root,
    );
    expect(ark.links.size).toBe(2);
    await ark.api.deleteBubble(root);
    expect(ark.links.size).toBe(0);
    const reloaded = await ark.api.listBubbles();
    expect(reloaded.map((bubble) => bubble.id).sort()).toEqual([child, sibling].sort());
    expect(reloaded.every((bubble) => !bubble.parentId)).toBe(true);
    expect(ark.calls).toContain("delete_object");
  });

  test("deleting a child keeps root and sibling intact", async () => {
    const ark = fakeArk();
    const root = await ark.api.createBubble("Корень");
    const child = await ark.api.createBubble("Ответ", "plain", root);
    const sibling = await ark.api.createBubble("Второй", "plain", root);
    await ark.api.deleteBubble(child);

    const reloaded = await ark.api.listBubbles();
    expect(reloaded.map((bubble) => bubble.id)).toEqual([root, sibling]);
    expect(reloaded[1]?.parentId).toBe(root);
  });

  test("migration is idempotent and rejects ambiguous timestamps", async () => {
    const ark = fakeArk();
    const source = {
      id: "legacy",
      date: "2026-07-01",
      time: "09:41",
      text: "Legacy",
      contentJson: {
        type: "doc",
        content: [{ type: "codeBlock", content: [{ type: "text", text: "Legacy" }] }],
      },
      tags: ["one", "one"],
      kind: "highlight" as const,
    };
    const first = await ark.api.migrateBubble("local", source.id, source);
    const second = await ark.api.migrateBubble("local", source.id, source);
    expect(second).toBe(first);
    expect([...ark.objects.values()].filter((object) => object.title === "Legacy")).toHaveLength(1);
    const migrated = ark.objects.get(first);
    expect(migrated).toBeDefined();
    expect(
      (migrated!.contentJson as { doc?: { content?: Array<{ type?: string }> } }).doc?.content?.[0]
        ?.type,
    ).toBe("codeBlock");
    await expect(
      ark.api.migrateBubble("local", "ambiguous", {
        ...source,
        id: "ambiguous",
        date: undefined,
      }),
    ).rejects.toThrow("Unresolved");
  });

  test("migration read-back ignores JSON object key order", async () => {
    const ark = fakeArk({ reorderJsonOnRead: true });
    const source = {
      id: "legacy-reordered-json",
      date: "2026-07-01",
      time: "09:41",
      text: "Rich legacy",
      contentJson: {
        type: "doc",
        attrs: { beta: 2, alpha: 1 },
        content: [
          {
            type: "paragraph",
            attrs: { zeta: true, gamma: false },
            content: [{ type: "text", text: "Rich legacy" }],
          },
        ],
      },
      tags: ["rich"],
      kind: "idea" as const,
    };

    const first = await ark.api.migrateBubble("local", source.id, source);
    const second = await ark.api.migrateBubble("local", source.id, source);
    expect(second).toBe(first);
    expect(ark.objects.size).toBe(1);
  });
});
