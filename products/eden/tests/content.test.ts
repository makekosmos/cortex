import { describe, expect, test } from "bun:test";
import {
  isMarkdownContent,
  legacyProseMirrorToText,
  readEntryMarkdown,
  writeEntryMarkdown,
} from "../src/editor-cm/content";

describe("editor-cm/content", () => {
  test("markdown content reads exact text and writes the markdown envelope", () => {
    const text = "# Заголовок\n\n- [ ] задача\n\nстрока  ";
    const content = writeEntryMarkdown(text);

    expect(content).toEqual({ type: "markdown", version: 1, text });
    expect(isMarkdownContent(content)).toBe(true);
    expect(readEntryMarkdown(content)).toBe(text);
    expect(readEntryMarkdown(JSON.stringify(content))).toBe(text);
    expect(writeEntryMarkdown(readEntryMarkdown(content)).text).toBe(text);
  });

  test("invalid and empty content reads as empty markdown without throwing", () => {
    for (const value of [null, undefined, "", "{not-json", {}, { type: "unknown" }]) {
      expect(() => readEntryMarkdown(value)).not.toThrow();
      expect(readEntryMarkdown(value)).toBe("");
    }
  });

  test("legacy ProseMirror JSON extracts text and media markdown best-effort", () => {
    const legacy = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Привет" }],
        },
        {
          type: "paragraph",
          content: [
            { type: "text", text: "строка" },
            { type: "hardBreak" },
            { type: "text", text: "после" },
          ],
        },
        { type: "image", attrs: { src: "file:///vault/pic.png" } },
        { type: "unknownWrapper", content: [{ type: "text", text: "nested" }] },
      ],
    };

    const markdown = legacyProseMirrorToText(legacy);
    expect(markdown).toContain("Привет");
    expect(markdown).toContain("строка\nпосле");
    expect(markdown).toContain("![](file:///vault/pic.png)");
    expect(markdown).toContain("nested");
  });

  test("image-only legacy content preserves source attributes", () => {
    expect(
      legacyProseMirrorToText({
        type: "doc",
        content: [{ type: "image", attrs: { url: "https://x/y.png" } }],
      }),
    ).toBe("![](https://x/y.png)");
  });
});
