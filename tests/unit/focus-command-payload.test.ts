import { describe, expect, test } from "bun:test";
import { reactive } from "vue";
import { buildFocusSessionStartInput } from "../../shell/src/components/focusCommandPayload";

describe("focus command payload", () => {
  test("builds clone-safe payload from reactive blocklist ids", () => {
    // Regression: 2026-06-05. Electron IPC cannot structured-clone Vue Proxy arrays.
    const categoryIds = reactive(["social", "video"]);
    const blockedAppIds = reactive(["steam", "discord"]);
    const blockedApps = reactive([
      { id: "steam", name: "Steam", icon: null, exec_path: "C:/Steam/steam.exe" },
    ]);

    const payload = buildFocusSessionStartInput({
      title: "",
      durationMin: 25,
      taskId: null,
      taskTitle: null,
      categoryIds,
      blockedAppIds,
      blockedApps,
    });

    expect(payload.categoryIds).toEqual(["social", "video"]);
    expect(payload.blockedAppIds).toEqual(["steam", "discord"]);
    expect(payload.blockedApps).toEqual([
      { id: "steam", name: "Steam", icon: null, exec_path: "C:/Steam/steam.exe" },
    ]);
    expect(() => structuredClone(payload)).not.toThrow();
  });
});
