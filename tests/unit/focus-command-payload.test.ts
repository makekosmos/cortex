import { describe, expect, test } from "bun:test";
import { reactive } from "vue";
import { buildFocusSessionStartInput } from "../../shell/src/components/focusCommandPayload";

describe("focus command payload", () => {
  test("builds clone-safe payload from reactive blocklist ids", () => {
    // Regression: 2026-06-05. Electron IPC cannot structured-clone Vue Proxy arrays.
    const categoryIds = reactive(["social", "video"]);

    const payload = buildFocusSessionStartInput({
      title: "",
      durationMin: 25,
      taskId: null,
      taskTitle: null,
      categoryIds,
    });

    expect(payload.categoryIds).toEqual(["social", "video"]);
    expect(() => structuredClone(payload)).not.toThrow();
  });
});
