import { describe, test, expect } from "bun:test";
import { isCmSafeDoc, shouldUseCmEditor } from "../src/editor-cm/cmGate";

describe("cmGate — isCmSafeDoc", () => {
  test("возвращает true для safe doc с heading + paragraph + list + codeBlock", () => {
    const safeDoc = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Привет" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "жирный", marks: [{ type: "bold" }] }],
        },
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [{ type: "paragraph", content: [{ type: "text", text: "пункт" }] }],
            },
          ],
        },
        {
          type: "codeBlock",
          attrs: { language: "ts" },
          content: [{ type: "text", text: "const a=1" }],
        },
      ],
    };
    expect(isCmSafeDoc(safeDoc)).toBe(true);
  });

  test("возвращает false если в глубине есть taskRef", () => {
    const docWithTaskRef = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "taskRef", attrs: { id: "task-123" } }],
        },
      ],
    };
    expect(isCmSafeDoc(docWithTaskRef)).toBe(false);
  });

  test("возвращает false если в глубине есть wikilink", () => {
    const docWithWikilink = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "wikilink", attrs: { target: "Some Page" } }],
        },
      ],
    };
    expect(isCmSafeDoc(docWithWikilink)).toBe(false);
  });

  test("возвращает false если в глубине есть unknownNode", () => {
    const docWithUnknown = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "unknownNode" }],
        },
      ],
    };
    expect(isCmSafeDoc(docWithUnknown)).toBe(false);
  });

  test("возвращает false если у текста mark type:unknownMark", () => {
    const docWithUnknownMark = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "текст", marks: [{ type: "unknownMark" }] }],
        },
      ],
    };
    expect(isCmSafeDoc(docWithUnknownMark)).toBe(false);
  });

  test("возвращает false для null", () => {
    expect(isCmSafeDoc(null)).toBe(false);
  });

  test("возвращает false для строки", () => {
    expect(isCmSafeDoc("string")).toBe(false);
  });

  test("возвращает false для числа", () => {
    expect(isCmSafeDoc(42)).toBe(false);
  });
});

describe("cmGate — shouldUseCmEditor", () => {
  test("возвращает false если prefEnabled === false, даже с safe JSON", () => {
    const safeJson = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "hello" }] }],
    });
    expect(shouldUseCmEditor(false, safeJson)).toBe(false);
  });

  test("возвращает true если prefEnabled === true и JSON safe", () => {
    const safeJson = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "hello" }] }],
    });
    expect(shouldUseCmEditor(true, safeJson)).toBe(true);
  });

  test("возвращает false если prefEnabled === true но JSON содержит taskRef", () => {
    const jsonWithTaskRef = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "taskRef", attrs: { id: "task-1" } }],
        },
      ],
    });
    expect(shouldUseCmEditor(true, jsonWithTaskRef)).toBe(false);
  });

  test("возвращает false если JSON невалидный", () => {
    expect(shouldUseCmEditor(true, "не json {")).toBe(false);
  });
});
