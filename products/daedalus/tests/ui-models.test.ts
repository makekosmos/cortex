import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import type { AgentsApproval, AgentsDiffFile } from "@kosmos/ark/agents";
import ChangesPanel from "@/components/ChangesPanel.vue";
import SessionTimeline from "@/components/SessionTimeline.vue";
import { buildChangeTree } from "@/changeTree";

describe("Daedalus UI models", () => {
  it("builds stable hierarchical change rows", () => {
    const files = [
      { path: "src/components/App.vue", status: "M" },
      { path: "README.md", status: "A" },
      { path: "src/main.ts", status: "M" },
    ].map((file) => ({
      ...file,
      size: 1,
      binary: false,
      contentOmitted: false,
    })) satisfies AgentsDiffFile[];
    expect(buildChangeTree(files).map((row) => `${row.depth}:${row.kind}:${row.name}`)).toEqual([
      "0:file:README.md",
      "0:directory:src",
      "1:directory:components",
      "2:file:App.vue",
      "1:file:main.ts",
    ]);
  });

  it("collects choices and free-text answers for multiple questions", async () => {
    const approval = {
      id: "approval",
      sessionId: "s",
      requestId: 7,
      method: "item/tool/requestUserInput",
      params: {
        questions: [
          {
            id: "scope",
            header: "Область",
            question: "Что менять?",
            options: [
              { label: "Только UI", description: "Без backend" },
              { label: "Всё", description: "UI и backend" },
            ],
          },
          { id: "note", header: "Комментарий", question: "Что учесть?" },
        ],
      },
      status: "pending",
      response: null,
      createdAt: "2026-01-01T00:00:00Z",
      resolvedAt: null,
    } satisfies AgentsApproval;
    const wrapper = mount(SessionTimeline, {
      props: { events: [], approval, hasOlder: false, loadingOlder: false },
    });
    await wrapper.get('input[type="radio"][value="Только UI"]').setValue();
    await wrapper.get('input[aria-label="Что учесть?"]').setValue("Сохранить стиль");
    await wrapper.get("button.primary").trigger("click");
    expect(wrapper.emitted("respond")?.[0]).toEqual([
      "approval",
      "accept",
      {
        scope: { answers: ["Только UI"] },
        note: { answers: ["Сохранить стиль"] },
      },
    ]);
  });

  it("opens the editor explicitly selected by the user", async () => {
    const wrapper = mount(ChangesPanel, {
      props: {
        diff: null,
        worktreeExists: true,
        editors: [
          { id: "code", label: "Visual Studio Code" },
          { id: "explorer", label: "Проводник" },
        ],
      },
    });
    await wrapper.get('select[aria-label="Редактор"]').setValue("explorer");
    await wrapper.get("button.secondary").trigger("click");
    expect(wrapper.emitted("open")?.[0]).toEqual(["explorer"]);
  });
});
