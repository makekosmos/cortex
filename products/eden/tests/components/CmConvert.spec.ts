// @vitest-environment jsdom

import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";
import { createMdConverter } from "../../src/editor-cm/mdConvert";

describe("CmConvert — Markdown ↔ PM JSON converter", () => {
  let converter: ReturnType<typeof createMdConverter>;

  beforeEach(() => {
    converter = createMdConverter();
  });

  afterEach(() => {
    converter.destroy();
  });

  test("jsonToMarkdown: PM JSON с heading level 1 → markdown содержит # Заголовок", () => {
    const pmJson = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Заголовок" }],
        },
      ],
    };
    const md = converter.jsonToMarkdown(pmJson);
    expect(md).toContain("# Заголовок");
  });

  test("jsonToMarkdown: paragraph с bold-текстом → markdown содержит **", () => {
    const pmJson = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "жирный", marks: [{ type: "bold" }] }],
        },
      ],
    };
    const md = converter.jsonToMarkdown(pmJson);
    expect(md).toContain("**");
  });

  test("markdownToJson: «# Привет\\n\\nтекст» → объект с heading level 1", () => {
    const md = "# Привет\n\nтекст";
    const json = converter.markdownToJson(md);
    expect(json).toHaveProperty("type", "doc");
    expect(Array.isArray(json.content)).toBe(true);
    // Первая нода должна быть heading level 1
    if (Array.isArray(json.content) && json.content.length > 0) {
      const firstNode = json.content[0];
      expect(firstNode.type).toBe("heading");
      expect(firstNode.attrs?.level).toBe(1);
    }
  });

  test("round-trip стабильность: markdown → JSON → markdown → JSON дают deep-equal JSON", () => {
    const mdFixture = `# Заголовок

Абзац с **жирным** и *курсивом* и \`кодом\`.

- пункт один
- пункт два

1. первый
2. второй

> цитата

\`\`\`ts
const a = 1;
\`\`\``;

    // Первый проход: markdown → JSON
    const json1 = converter.markdownToJson(mdFixture);

    // Второй проход: JSON → markdown
    const md2 = converter.jsonToMarkdown(json1);

    // Третий проход: markdown → JSON
    const json2 = converter.markdownToJson(md2);

    // Сравниваем JSON1 и JSON2 (не сырые строки markdown)
    expect(JSON.stringify(json2)).toEqual(JSON.stringify(json1));
  });

  test("round-trip чекбокс-строк: markdown с [ ] и [x] сохраняют подстроки", () => {
    const md = "- [ ] не сделано\n- [x] сделано";

    const json1 = converter.markdownToJson(md);
    const md2 = converter.jsonToMarkdown(json1);

    // Проверяем что подстроки "[ ]" и "[x]" сохранены в markdown
    expect(md2).toContain("[ ]");
    expect(md2).toContain("[x]");
  });

  test("round-trip изображения: markdown image syntax сохраняется", () => {
    const md = "До\n\n![Обложка](file:///C:/vault/attachments/cover.png)\n\nПосле";

    const json = converter.markdownToJson(md);
    const md2 = converter.jsonToMarkdown(json);

    expect(md2).toContain("![Обложка](file:///C:/vault/attachments/cover.png)");
  });
});
