import { describe, expect, test } from "vitest";
import { readEntryMarkdown, writeEntryMarkdown } from "../../src/editor-cm/content";

describe("CM markdown storage adapter", () => {
  test("читает markdown storage без конвертации", () => {
    const text = "# Заголовок\n\n**жирный**";
    expect(readEntryMarkdown(writeEntryMarkdown(text))).toBe(text);
  });
});
