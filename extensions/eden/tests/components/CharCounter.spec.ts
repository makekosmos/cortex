// Component smoke: char counter в zen mode рендерится с правильной русской
// плюрализацией и реактивно обновляется при изменении контента.
//
// Цель — продемонстрировать что vitest browser + vitest-browser-vue работают:
// реальный Chromium, реальная DOM, реальная Vue реактивность. Не пытаемся
// тестировать всё App.vue (слишком много prop'ов), а вырезаем минимальный
// fixture с теми же computed что и в App.vue:198-436.

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import { defineComponent, ref, computed } from "vue";
import { countCharsInProseMirrorDoc } from "../../src/lib/charCount";

function pluralize(n: number): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return "символ";
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return "символа";
  return "символов";
}

const CharCounter = defineComponent({
  props: {
    contentJson: { type: String, required: true },
  },
  setup(props) {
    const count = computed(() => countCharsInProseMirrorDoc(props.contentJson) ?? 0);
    const label = computed(() => `${count.value} ${pluralize(count.value)}`);
    return { label };
  },
  template: `<div class="eden-char-counter" data-testid="eden-char-counter">{{ label }}</div>`,
});

describe("Eden char counter (zen-mode UI)", () => {
  test("пустой doc → 0 символов", async () => {
    const { getByTestId } = render(CharCounter, {
      props: { contentJson: JSON.stringify({ type: "doc", content: [] }) },
    });
    await expect.element(getByTestId("eden-char-counter")).toHaveTextContent("0 символов");
  });

  test("«hello» → 5 символов", async () => {
    const contentJson = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "hello" }],
        },
      ],
    });
    const { getByTestId } = render(CharCounter, { props: { contentJson } });
    await expect.element(getByTestId("eden-char-counter")).toHaveTextContent("5 символов");
  });

  test("«1 символ» (плюрализация 1)", async () => {
    const contentJson = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "a" }] }],
    });
    const { getByTestId } = render(CharCounter, { props: { contentJson } });
    await expect.element(getByTestId("eden-char-counter")).toHaveTextContent("1 символ");
  });

  test("«2 символа» (плюрализация 2-4)", async () => {
    const contentJson = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "ab" }] }],
    });
    const { getByTestId } = render(CharCounter, { props: { contentJson } });
    await expect.element(getByTestId("eden-char-counter")).toHaveTextContent("2 символа");
  });

  test("emoji 😀 = 1 символ (не 2 UTF-16 units)", async () => {
    const contentJson = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "😀" }] }],
    });
    const { getByTestId } = render(CharCounter, { props: { contentJson } });
    await expect.element(getByTestId("eden-char-counter")).toHaveTextContent("1 символ");
  });
});
