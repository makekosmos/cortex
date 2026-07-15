import { describe, expect, test } from "bun:test";
import { classifyFpsDrop, shouldRecordFpsDrop } from "../src/composables/useFpsMonitor";

describe("FPS monitor", () => {
  test("records meaningful drops and preserves honest attribution", () => {
    expect(shouldRecordFpsDrop(44, 60)).toBe(true);
    expect(shouldRecordFpsDrop(45, 60)).toBe(false);
    expect(classifyFpsDrop(false, 81)).toBe("long-task");
    expect(classifyFpsDrop(true, 81)).toBe("page-hidden");
    expect(classifyFpsDrop(false, 0)).toBe("unknown");
  });
});
