// Component smoke: MentionMenu — pure-render компонент списка задач без
// runtime deps (window.delphi / store). Цель — продемонстрировать что
// vitest browser работает в horologion (Phase 6 bug-detection).

import { describe, expect, test } from "vitest";
import { render } from "vitest-browser-vue";
import MentionMenu from "../../src/components/MentionMenu.vue";
import type { DelphiTask } from "../../src/types";

const sampleTasks: DelphiTask[] = [
  { id: "t1", title: "Написать спецификацию", status: null },
  { id: "t2", title: "Пройти ревью", status: null },
  { id: "t3", title: "Закрыть PR", status: null },
];

describe("Horologion MentionMenu", () => {
  test("closed → ничего не рендерит", async () => {
    const { container } = render(MentionMenu, {
      props: { open: false, query: "", tasks: sampleTasks, highlightedIndex: 0 },
    });
    expect(container.querySelector(".mention-menu")).toBeNull();
  });

  test("open, пустой query → рендерит все задачи (до 8)", async () => {
    const { container } = render(MentionMenu, {
      props: { open: true, query: "", tasks: sampleTasks, highlightedIndex: 0 },
    });
    const items = container.querySelectorAll(".mention-menu__item");
    expect(items.length).toBe(3);
  });

  test("query фильтрует по title", async () => {
    const { container } = render(MentionMenu, {
      props: { open: true, query: "ревью", tasks: sampleTasks, highlightedIndex: 0 },
    });
    const items = container.querySelectorAll(".mention-menu__item");
    expect(items.length).toBe(1);
    expect(items[0]?.textContent).toContain("ревью");
  });

  test("highlight применяется к нужному индексу", async () => {
    const { container } = render(MentionMenu, {
      props: { open: true, query: "", tasks: sampleTasks, highlightedIndex: 1 },
    });
    const items = container.querySelectorAll(".mention-menu__item");
    expect(items[1]?.className).toMatch(/mention-menu__item--active/);
    expect(items[0]?.className).not.toMatch(/mention-menu__item--active/);
  });
});
