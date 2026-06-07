// Component smoke: маленькая обёртка над formatDuration() — демонстрирует
// что Vue реактивность работает через секундный счётчик. Тут проверяем не
// сам helper (он покрыт unit'ом), а интеграцию вычисляемого свойства с
// рендерингом в реальном Chromium (Phase 6 bug-detection).

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import { defineComponent, ref, computed } from "vue";
import { formatDuration } from "../../src/lib/format";

const DurationLabel = defineComponent({
  props: {
    seconds: { type: Number, required: true },
  },
  setup(props) {
    const label = computed(() => formatDuration(props.seconds));
    return { label };
  },
  template: `<span data-testid="duration">{{ label }}</span>`,
});

describe("Horologion DurationLabel (smoke)", () => {
  test("0 секунд → 0:00:00", async () => {
    const { getByTestId } = render(DurationLabel, { props: { seconds: 0 } });
    await expect.element(getByTestId("duration")).toHaveTextContent("0:00:00");
  });

  test("3661 секунда → 1:01:01", async () => {
    const { getByTestId } = render(DurationLabel, { props: { seconds: 3661 } });
    await expect.element(getByTestId("duration")).toHaveTextContent("1:01:01");
  });

  test("реактивно обновляется через ref", async () => {
    const seconds = ref(0);
    const Wrapper = defineComponent({
      components: { DurationLabel },
      setup: () => ({ seconds }),
      template: `<DurationLabel :seconds="seconds" />`,
    });
    const { getByTestId } = render(Wrapper);
    await expect.element(getByTestId("duration")).toHaveTextContent("0:00:00");
    seconds.value = 65;
    await expect.element(getByTestId("duration")).toHaveTextContent("0:01:05");
  });
});
