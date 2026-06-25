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
    expect(root).not.toBeNull();
    expect(root?.className).toMatch(/animate-pulse/);
    expect(root?.className).toMatch(/rounded-md/);
  });

  test("проп class мерджится через cn()", async () => {
    const { container } = render(Skeleton, {
      props: { class: "h-4 w-32" },
    });
    const root = container.querySelector("div");
    expect(root?.className).toMatch(/h-4/);
    expect(root?.className).toMatch(/w-32/);
  });
});
