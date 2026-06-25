// Component smoke: Skeleton — pure-render компонент с animate-pulse класс.
// Реальный Chromium, реальный CSS — sanity check что Tailwind utilities
// рендерятся без runtime ошибок.

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import Skeleton from "../../src/components/Skeleton.vue";

describe("Delphi Skeleton", () => {
  test("монтируется с базовыми классами", async () => {
    const { container } = render(Skeleton);
    const root = container.querySelector("div");
    if (!root) throw new Error("expected Skeleton root element");
    expect(root.classList.contains("animate-pulse")).toBe(true);
    expect(root.classList.contains("rounded-[var(--radius-input)]")).toBe(true);
  });

  test("проп class мерджится через cn()", async () => {
    const { container } = render(Skeleton, {
      props: { class: "h-4 w-32" },
    });
    const root = container.querySelector("div");
    if (!root) throw new Error("expected Skeleton root element");
    expect(root.classList.contains("h-4")).toBe(true);
    expect(root.classList.contains("w-32")).toBe(true);
  });
});
