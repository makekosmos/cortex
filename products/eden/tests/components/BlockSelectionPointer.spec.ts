import { describe, expect, test } from "vitest";
import { shouldStartBlockSelectionTracking } from "../../src/lib/blockSelectionPointer";

function createEditorDom() {
  const contentArea = document.createElement("div");
  contentArea.className = "editor-content-area";

  const rail = document.createElement("div");
  rail.className = "editor-rail";

  const proseMirror = document.createElement("div");
  proseMirror.className = "ProseMirror";

  const paragraph = document.createElement("p");
  const inline = document.createElement("span");
  inline.textContent = "Alpha beta gamma";
  paragraph.append(inline);

  const button = document.createElement("button");
  button.textContent = "Action";

  const input = document.createElement("input");
  input.className = "task-ref-title-input";

  proseMirror.append(paragraph, button, input);
  rail.append(proseMirror);
  contentArea.append(rail);

  return { contentArea, rail, proseMirror, paragraph, inline, button, input };
}

describe("block-selection pointer gate", () => {
  test("keeps native text selection when drag starts inside ProseMirror text", () => {
    // Regression: 2026-05-28. Text drag from the top of Eden notes must not
    // enter block-selection mode, which owns auto-scroll.
    const { contentArea, paragraph, inline } = createEditorDom();

    expect(shouldStartBlockSelectionTracking(paragraph, contentArea)).toBe(false);
    expect(shouldStartBlockSelectionTracking(inline, contentArea)).toBe(false);
    expect(shouldStartBlockSelectionTracking(inline.firstChild, contentArea)).toBe(false);
  });

  test("allows block selection from editor gutter and empty ProseMirror surface", () => {
    const { contentArea, rail, proseMirror } = createEditorDom();

    expect(shouldStartBlockSelectionTracking(contentArea, contentArea)).toBe(true);
    expect(shouldStartBlockSelectionTracking(rail, contentArea)).toBe(true);
    expect(shouldStartBlockSelectionTracking(proseMirror, contentArea)).toBe(true);
  });

  test("does not start block selection from interactive controls", () => {
    const { contentArea, button, input } = createEditorDom();

    expect(shouldStartBlockSelectionTracking(button, contentArea)).toBe(false);
    expect(shouldStartBlockSelectionTracking(input, contentArea)).toBe(false);
  });
});
