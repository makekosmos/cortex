import { describe, expect, test } from "bun:test";
import { VIM_MOTION_GROUPS } from "../src/editor-cm/vimMotions";

describe("vimMotions", () => {
  test("публикует группы движений и команды Eden для страницы настроек", () => {
    expect(VIM_MOTION_GROUPS.length).toBeGreaterThanOrEqual(5);
    expect(VIM_MOTION_GROUPS.every((group) => group.title.trim().length > 0)).toBe(true);
    expect(VIM_MOTION_GROUPS.every((group) => group.items.length > 0)).toBe(true);
    expect(
      VIM_MOTION_GROUPS.every((group) =>
        group.items.every(
          (item) =>
            item.keys.trim().length > 0 &&
            item.title.trim().length > 0 &&
            item.description.trim().length > 0,
        ),
      ),
    ).toBe(true);

    const allKeys = VIM_MOTION_GROUPS.flatMap((group) => group.items.map((item) => item.keys));
    expect(new Set(allKeys).size).toBe(allKeys.length);
    expect(allKeys).toContain(":w");
    expect(allKeys).toContain(":q");
    expect(allKeys).toContain(":wq");
    expect(allKeys).toContain(":zen");
    expect(allKeys).toContain(":zen on");
    expect(allKeys).toContain(":zen off");
    expect(allKeys).toContain("Ctrl+v");
  });
});
