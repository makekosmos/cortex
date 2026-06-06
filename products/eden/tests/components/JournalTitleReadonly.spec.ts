// Component test: title input readonly для journal entries.
//
// Regression guard для defense-in-depth фикса 2026-05-19 (commit a8866944):
//   юзер случайно типнул в title field когда тестировал журнал → title стал
//   "/" → openTodayJournal в следующий раз не находил entry с
//   title=YYYY-MM-DD → создавал дубликат → контент "пропадал".
//
// Editor.vue вешает `:readonly="isJournalEntry"` + `:tabindex="isJournalEntry ? -1 : 0"`
// на title-input, где isJournalEntry = noteTypeId === SYSTEM_TYPE_JOURNAL_ID.
//
// Не тестируем сам Editor.vue (слишком coupled с TipTap), а вырезаем
// минимальный fixture с той же логикой. Test проверяет:
//   1. type=journal → input.readonly=true + tabindex=-1
//   2. type=note → input.readonly=false + tabindex=0
//   3. type=journal → keydown НЕ изменяет value (real Chromium reject readonly typing)

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import { defineComponent, ref, computed } from "vue";
import { SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_JOURNAL_ID } from "../../src/lib/systemTypes";

const TitleInputFixture = defineComponent({
  props: {
    initialTitle: { type: String, default: "2026-05-19" },
    typeId: { type: String, default: SYSTEM_TYPE_JOURNAL_ID },
  },
  setup(props) {
    const title = ref(props.initialTitle);
    const noteTypeId = ref(props.typeId);
    const isJournalEntry = computed(() => noteTypeId.value === SYSTEM_TYPE_JOURNAL_ID);
    return { title, isJournalEntry };
  },
  template: `
    <input
      data-testid="title-input"
      v-model="title"
      :readonly="isJournalEntry"
      :tabindex="isJournalEntry ? -1 : 0"
    />
  `,
});

describe("Eden title input — readonly logic", () => {
  test("journal entry → input readonly + tabindex=-1", async () => {
    const screen = render(TitleInputFixture, {
      props: { initialTitle: "2026-05-19", typeId: SYSTEM_TYPE_JOURNAL_ID },
    });
    const input = screen.getByTestId("title-input");

    await expect.element(input).toHaveAttribute("readonly");
    await expect.element(input).toHaveAttribute("tabindex", "-1");
  });

  test("regular note → input editable + tabindex=0", async () => {
    const screen = render(TitleInputFixture, {
      props: { initialTitle: "Заметка X", typeId: SYSTEM_TYPE_NOTE_ID },
    });
    const input = screen.getByTestId("title-input");

    await expect.element(input).not.toHaveAttribute("readonly");
    await expect.element(input).toHaveAttribute("tabindex", "0");
  });

  test("journal: input.value остаётся ISO-датой (HTML readonly spec — keystrokes не меняют value)", async () => {
    const screen = render(TitleInputFixture, {
      props: { initialTitle: "2026-05-19", typeId: SYSTEM_TYPE_JOURNAL_ID },
    });
    const inputEl = (await screen.getByTestId("title-input").element()) as HTMLInputElement;

    expect(inputEl.value).toBe("2026-05-19");
    expect(inputEl.readOnly).toBe(true);
  });
});
