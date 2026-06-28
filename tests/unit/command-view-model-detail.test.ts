import { describe, expect, test } from "bun:test";
import {
  Action,
  ActionPanel,
  Detail,
} from "../../packages/raycast-api/src/index";
import { normalizeCommandNode } from "../../platform/desktop/electron/command-host/view-model";
import { actionNodes } from "../../platform/desktop/src/command-host/model";
import {
  detailActions,
  detailMetadataItems,
  detailMarkdown,
} from "../../platform/desktop/src/command-host/model-detail";
import { parseCommandMarkdown } from "../../platform/desktop/src/command-host/markdown";

describe("Command detail view model", () => {
  test("parses Command Detail markdown into safe render blocks", () => {
    expect(
      parseCommandMarkdown("# Заголовок\n\nТекст\n\n- Один\n- Два\n\n```ts\nconst ok = true;\n```"),
    ).toEqual([
      { type: "heading", level: 1, text: "Заголовок" },
      { type: "paragraph", text: "Текст" },
      { type: "list", items: ["Один", "Два"] },
      { type: "code", text: "const ok = true;" },
    ]);
  });

  test("normalizes Detail.Metadata into host metadata rows", () => {
    const root = Detail({
      markdown: "# Проект",
      metadata: Detail.Metadata({
        children: [
          Detail.Metadata.Label({ title: "Статус", text: "Активен" }),
          Detail.Metadata.Link({
            title: "Ссылка",
            text: "Документация",
            target: "https://example.com/docs",
          }),
          Detail.Metadata.Separator({}),
          Detail.Metadata.TagList({
            title: "Теги",
            children: [
              Detail.Metadata.TagList.Item({ text: "raycast" }),
              Detail.Metadata.TagList.Item({ text: "kosmos" }),
            ],
          }),
        ],
      }),
    });

    const snapshot = normalizeCommandNode(root);
    expect(snapshot?.type).toBe("Detail");
    expect(snapshot ? detailMarkdown(snapshot) : "").toBe("# Проект");
    expect(snapshot ? detailMetadataItems(snapshot) : []).toEqual([
      { id: "metadata:0", type: "label", title: "Статус", text: "Активен", href: null },
      {
        id: "metadata:1",
        type: "link",
        title: "Ссылка",
        text: "Документация",
        href: "https://example.com/docs",
      },
      { id: "metadata:2", type: "separator" },
      { id: "metadata:3", type: "tags", title: "Теги", tags: ["raycast", "kosmos"] },
    ]);
  });

  test("normalizes root Detail actions without leaking pushed detail markdown", () => {
    const root = Detail({
      markdown: "# Проект",
      actions: ActionPanel({
        children: [
          Action.CopyToClipboard({ title: "Скопировать", content: "Проект" }),
          Action.Push({ title: "Открыть", target: Detail({ markdown: "# Вложенный экран" }) }),
        ],
      }),
    });

    const snapshot = normalizeCommandNode(root);
    expect(snapshot?.children.map((node) => node.type)).toEqual(["ActionPanel"]);
    expect(snapshot ? detailMarkdown(snapshot) : "").toBe("# Проект");
    expect(actionNodes(snapshot ? detailActions(snapshot) : null).map((node) => node.type)).toEqual(
      ["Action.CopyToClipboard", "Action.Push"],
    );
  });
});
