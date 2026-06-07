// Component test: Eden title-related helpers + their rendering.
//
// Покрывает:
//   - getEntryDisplayTitle: показывает либо реальный title, либо placeholder
//     «Без названия» когда title пустой или есть __untitledTitle flag.
//   - getEditableEntryTitle: возвращает "" если is-untitled (чтобы placeholder
//     отрисовался в input), либо реальный title.
//
// Был баг (commit c82fa2bf): journal entry имела __untitledTitle=true, и UI
// показывал «Без названия» вместо реальной даты "2026-05-19". С тех пор
// openTodayJournal явно ставит header_props_json: "{}" (без flag'а).

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import { defineComponent, computed } from "vue";
import { getEntryDisplayTitle, getEditableEntryTitle } from "../../src/lib/entryTitles";

const DisplayFixture = defineComponent({
  props: {
    title: { type: String, default: "" },
    headerPropsJson: { type: String, default: "{}" },
  },
  setup(props) {
    const displayed = computed(() => getEntryDisplayTitle(props.title, props.headerPropsJson));
    const editable = computed(() => getEditableEntryTitle(props.title, props.headerPropsJson));
    return { displayed, editable };
  },
  template: `
    <div>
      <span data-testid="displayed">{{ displayed }}</span>
      <input data-testid="editable" :value="editable" readonly />
    </div>
  `,
});

describe("Entry title helpers", () => {
  test("обычный title без flag → отображается as-is", async () => {
    const screen = render(DisplayFixture, {
      props: { title: "Моя заметка", headerPropsJson: "{}" },
    });
    await expect.element(screen.getByTestId("displayed")).toHaveTextContent("Моя заметка");
    const editable = (await screen.getByTestId("editable").element()) as HTMLInputElement;
    expect(editable.value).toBe("Моя заметка");
  });

  test("пустой title без flag → placeholder в display, '' в editable", async () => {
    const screen = render(DisplayFixture, {
      props: { title: "", headerPropsJson: "{}" },
    });
    await expect.element(screen.getByTestId("displayed")).toHaveTextContent("Без названия");
    const editable = (await screen.getByTestId("editable").element()) as HTMLInputElement;
    expect(editable.value).toBe("");
  });

  test("__untitledTitle=true + дата → display показывает placeholder, editable пустой", async () => {
    // Это сценарий старого бага (до c82fa2bf): journal с flag'ом
    // отображался как «Без названия» вместо даты.
    const screen = render(DisplayFixture, {
      props: {
        title: "2026-05-19",
        headerPropsJson: JSON.stringify({ __untitledTitle: true }),
      },
    });
    await expect.element(screen.getByTestId("displayed")).toHaveTextContent("Без названия");
    const editable = (await screen.getByTestId("editable").element()) as HTMLInputElement;
    expect(editable.value).toBe("");
  });

  test("обычный title с __untitledTitle: false → отображается as-is", async () => {
    const screen = render(DisplayFixture, {
      props: {
        title: "Хорошее название",
        headerPropsJson: JSON.stringify({ __untitledTitle: false }),
      },
    });
    await expect.element(screen.getByTestId("displayed")).toHaveTextContent("Хорошее название");
  });

  test("battered header_props_json (invalid JSON) → не падает, fallback на title", async () => {
    const screen = render(DisplayFixture, {
      props: {
        title: "Что-то осмысленное",
        headerPropsJson: "{not-json,,,",
      },
    });
    // Не должно крашиться, должен отрисовать title.
    await expect.element(screen.getByTestId("displayed")).toHaveTextContent("Что-то осмысленное");
  });
});
