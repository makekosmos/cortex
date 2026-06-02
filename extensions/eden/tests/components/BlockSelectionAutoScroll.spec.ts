import { describe, expect, test } from "vitest";
import { computeBlockSelectionAutoScrollDelta } from "../../src/composables/useBlockSelection";

function createScrollContainer(top: number, bottom: number): HTMLElement {
  const element = document.createElement("div");
  element.getBoundingClientRect = () =>
    ({
      top,
      bottom,
      left: 0,
      right: 400,
      width: 400,
      height: bottom - top,
      x: 0,
      y: top,
      toJSON: () => ({}),
    }) as DOMRect;
  return element;
}

describe("block-selection edge autoscroll", () => {
  test("does not scroll while pointer is away from scroll-container edges", () => {
    const container = createScrollContainer(100, 500);

    expect(computeBlockSelectionAutoScrollDelta(250, container)).toBe(0);
  });

  test("scrolls up only in the top edge zone", () => {
    const container = createScrollContainer(100, 500);

    expect(computeBlockSelectionAutoScrollDelta(108, container)).toBeLessThan(0);
  });

  test("scrolls down only in the bottom edge zone", () => {
    const container = createScrollContainer(100, 500);

    expect(computeBlockSelectionAutoScrollDelta(492, container)).toBeGreaterThan(0);
  });
});
