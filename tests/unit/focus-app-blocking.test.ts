import { describe, expect, test } from "bun:test";
import { isAppBlockedByFocus } from "../../platform/desktop/src/lib/focusAppBlocking";

describe("focus app blocking", () => {
  test("blocks selected app ids only while focus is active", () => {
    expect(
      isAppBlockedByFocus({ active: true, blocked_app_ids: ["steam", "discord"] }, "steam"),
    ).toBe(true);
    expect(
      isAppBlockedByFocus({ active: true, blocked_app_ids: ["steam", "discord"] }, "notepad"),
    ).toBe(false);
    expect(isAppBlockedByFocus({ active: false, blocked_app_ids: ["steam"] }, "steam")).toBe(false);
  });

  test("blocks launcher apps by normalized display name", () => {
    // Regression: 2026-06-05. Focus blocking follows launcher-visible app names,
    // not only one technical app_index id.
    expect(
      isAppBlockedByFocus(
        { active: true, blocked_app_ids: [], blocked_apps: [{ id: "steam-a", name: "Steam" }] },
        "steam-b",
        "  STEAM  ",
      ),
    ).toBe(true);
  });
});
