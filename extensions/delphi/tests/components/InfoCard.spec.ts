// Component smoke: InfoCard рендерит slot-контент в обёртке с
// design-токеновыми классами. Цель — продемонстрировать что vitest
// browser работает в delphi (Phase 6 bug-detection), не тестировать
// visual presentation.

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import InfoCard from "../../src/components/InfoCard.vue";

describe("Delphi InfoCard", () => {
  test("монтируется и рендерит slot", async () => {
    const { getByText } = render(InfoCard, {
      slots: { default: "привет, мир" },
    });
    await expect.element(getByText("привет, мир")).toBeInTheDocument();
  });

  test("обёртка содержит ожидаемые токеновые классы", async () => {
    const { container } = render(InfoCard, {
      slots: { default: "содержимое" },
    });
    const root = container.querySelector("div");
    expect(root).not.toBeNull();
    // class содержит rounded-xl + bg-(--card) → проверяем хотя бы
    // первый, остальное может меняться с tailwind v4.
    expect(root?.className).toMatch(/rounded-xl/);
  });
});
