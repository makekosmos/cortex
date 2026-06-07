import { describe, expect, test } from "vitest";
import { applyBlockSelectionClasses } from "../../src/lib/blockSelectionClasses";

describe("block-selection classes", () => {
  test("separates persisted selection mode from active drag mode", () => {
    const element = document.createElement("div");

    applyBlockSelectionClasses(element, { hasSelection: true, isDragging: false });

    expect(element.classList.contains("kepler-block-select-active")).toBe(true);
    expect(element.classList.contains("kepler-block-drag-active")).toBe(false);
  });

  test("marks both selection and drag mode while rubber-band drag is active", () => {
    const element = document.createElement("div");

    applyBlockSelectionClasses(element, { hasSelection: false, isDragging: true });

    expect(element.classList.contains("kepler-block-select-active")).toBe(true);
    expect(element.classList.contains("kepler-block-drag-active")).toBe(true);
  });

  test("clears both classes after selection is cleared", () => {
    const element = document.createElement("div");
    element.classList.add("kepler-block-select-active", "kepler-block-drag-active");

    applyBlockSelectionClasses(element, { hasSelection: false, isDragging: false });

    expect(element.classList.contains("kepler-block-select-active")).toBe(false);
    expect(element.classList.contains("kepler-block-drag-active")).toBe(false);
  });
});
