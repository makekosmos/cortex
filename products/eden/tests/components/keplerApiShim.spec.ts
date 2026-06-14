import { describe, expect, test } from "vitest";
import { listEntries } from "@/lib/kepler-api-shim";

describe("kepler-api-shim timestamps", () => {
  test("listEntries does not turn missing updatedAt into current time", async () => {
    const createdAt = "2024-01-02T03:04:05.000Z";
    const requests: Record<string, unknown> = {
      list_objects: [
        {
          id: "old-note",
          typeId: "note_obj",
          title: "Старая заметка",
          contentJson: { type: "markdown", version: 1, text: "text" },
          propsJson: {},
          createdAt,
          updatedAt: null,
          deletedAt: null,
        },
      ],
      list_object_links: [],
      list_object_types: [],
    };

    const previousKepler = window.kepler;
    Object.defineProperty(window, "kepler", {
      configurable: true,
      value: {
        ark: {
          request: async (operation: string) => requests[operation],
          subscribe: () => () => undefined,
        },
      },
    });

    try {
      const [entry] = await listEntries();

      expect(entry.updated_at).toBe(Date.parse(createdAt));
      expect(entry.updated_at).toBe(entry.created_at);
    } finally {
      Object.defineProperty(window, "kepler", {
        configurable: true,
        value: previousKepler,
      });
    }
  });
});
