import { describe, expect, test } from "bun:test";
import {
  isMarkdownContent,
  isEntryTiptapContent,
  isReadableEntryContent,
  legacyProseMirrorToText,
  markdownToTiptapDoc,
  readEntryMarkdown,
  readEntryTiptapDoc,
  tiptapDocToMarkdown,
  writeEntryMarkdown,
  writeEntryTiptapDoc,
} from "../src/editor-content/content";

describe("editor-content/content", () => {
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
    expect(isReadableEntryContent("{not-json")).toBe(false);
    expect(isReadableEntryContent(writeEntryMarkdown(""))).toBe(true);
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
            { type: "text", text: "строка", marks: [{ type: "strong" }] },
            { type: "hardBreak" },
            {
              type: "text",
              text: "после",
              marks: [{ type: "link", attrs: { href: "https://example.com" } }],
            },
          ],
        },
        {
          type: "codeBlock",
          attrs: { language: "ts" },
          content: [{ type: "text", text: "const x = 1;" }],
        },
        { type: "image", attrs: { src: "file:///vault/pic.png" } },
        { type: "unknownWrapper", content: [{ type: "text", text: "nested" }] },
      ],
    };

    const markdown = legacyProseMirrorToText(legacy);
    expect(markdown).toContain("## Привет");
    expect(markdown).toContain("**строка**\n[после](https://example.com)");
    expect(markdown).toContain("```ts\nconst x = 1;\n```");
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

  test("markdown can be migrated to TipTap JSON and read back as markdown", () => {
    const markdown = [
      "# Title",
      "",
      "- [x] done",
      "- [ ] next",
      "",
      "1. first",
      "",
      "> quote",
      "",
      "```",
      "code",
      "```",
    ].join("\n");

    const doc = markdownToTiptapDoc(markdown);
    const content = writeEntryTiptapDoc(doc);

    expect(content.type).toBe("tiptap");
    expect(isEntryTiptapContent(JSON.stringify(content))).toBe(true);
    expect(readEntryTiptapDoc(writeEntryMarkdown(markdown))).toEqual(doc);
    expect(readEntryMarkdown(content)).toContain("- [x] done");
    expect(tiptapDocToMarkdown(doc)).toContain("```");
  });

  test("markdown round-trips common marks, links, code, images, lists, quotes, and fences", () => {
    const markdown = [
      "## Heading with **bold** and *italic*",
      "",
      'Paragraph with [link](https://example.com), `code`, and ![alt](https://cdn.test/img.png "caption")',
      "",
      "- bullet with ~~strike~~",
      "3. ordered",
      "- [x] task",
      "",
      "> quoted **text**",
      "",
      "```ts",
      "const value = 1;",
      "```",
    ].join("\n");

    const roundTrip = tiptapDocToMarkdown(markdownToTiptapDoc(markdown));

    expect(roundTrip).toContain("Paragraph with [link](https://example.com), `code`, and");
    expect(roundTrip).toContain('![alt](https://cdn.test/img.png "caption")');
    expect(roundTrip).toContain("- bullet with ~~strike~~");
    expect(roundTrip).toContain("```ts\nconst value = 1;\n```");
  });

  test("markdown inline image becomes a valid block image node", () => {
    const doc = markdownToTiptapDoc("before ![alt](https://cdn.test/img.png) after");

    expect(doc.content?.map((node) => node.type)).toEqual(["paragraph", "image", "paragraph"]);
    expect(doc.content?.[0]?.content?.[0]?.text).toBe("before ");
    expect(doc.content?.[2]?.content?.[0]?.text).toBe(" after");
  });

  test("markdown paragraph line breaks become hardBreak nodes instead of spaces", () => {
    const doc = readEntryTiptapDoc(writeEntryMarkdown("alpha\nbeta"));

    expect(doc.content?.[0]).toEqual({
      type: "paragraph",
      content: [
        { type: "text", text: "alpha" },
        { type: "hardBreak" },
        { type: "text", text: "beta" },
      ],
    });
  });

  test("markdown nested lists preserve child indentation", () => {
    const markdown = ["- parent", "  - child", "- sibling"].join("\n");

    expect(tiptapDocToMarkdown(markdownToTiptapDoc(markdown))).toBe(markdown);
  });

  test("code fences expand when code contains triple backticks", () => {
    const doc = {
      type: "doc",
      content: [
        {
          type: "codeBlock",
          content: [{ type: "text", text: "before\n```\nafter" }],
        },
      ],
    } satisfies ReturnType<typeof readEntryTiptapDoc>;

    const markdown = tiptapDocToMarkdown(doc);
    expect(markdown).toBe("````\nbefore\n```\nafter\n````");
    expect(readEntryTiptapDoc(writeEntryMarkdown(markdown))).toEqual(doc);
  });

  test("reads TipTap docs with inline marks and images back into markdown", () => {
    const doc = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            { type: "text", text: "bold", marks: [{ type: "bold" }] },
            { type: "text", text: " " },
            {
              type: "text",
              text: "link",
              marks: [{ type: "link", attrs: { href: "https://example.com" } }],
            },
          ],
        },
        {
          type: "image",
          attrs: { src: "https://cdn.test/pic.png", alt: "pic" },
        },
      ],
    } satisfies ReturnType<typeof readEntryTiptapDoc>;

    expect(readEntryMarkdown(writeEntryTiptapDoc(doc))).toBe(
      "**bold** [link](https://example.com)\n\n![pic](https://cdn.test/pic.png)",
    );
  });
});
