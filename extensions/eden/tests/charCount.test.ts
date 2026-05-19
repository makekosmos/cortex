// Regression-тесты для подсчёта символов в zen-mode counter Eden.
//
// Защита от:
// - UTF-16 surrogate pairs (emoji) — должен считаться 1 character, не 2.
// - Игнорирования paragraph breaks — каждый Enter между параграфами
//   = 1 видимый символ (newline).
// - Игнорирования hard_break (Shift+Enter) — тоже 1 символ.
// - Container'ы (lists, blockquote) сами по себе newline не вносят —
//   только их leaf-block содержимое.

import { describe, test, expect } from "bun:test";
import {
  countCharsInProseMirrorDoc,
  countCharsInProseMirrorNode,
} from "../src/lib/charCount.js";

function doc(...content: unknown[]) {
  return JSON.stringify({ type: "doc", content });
}

function p(...content: unknown[]) {
  return { type: "paragraph", content };
}

function t(text: string) {
  return { type: "text", text };
}

describe("countCharsInProseMirrorDoc", () => {
  test("returns null для null / undefined / пустой строки", () => {
    expect(countCharsInProseMirrorDoc(null)).toBe(null);
    expect(countCharsInProseMirrorDoc(undefined)).toBe(null);
    expect(countCharsInProseMirrorDoc("")).toBe(null);
  });

  test("returns null если JSON битый", () => {
    expect(countCharsInProseMirrorDoc("{not json")).toBe(null);
  });

  test("пустой doc → 0", () => {
    expect(countCharsInProseMirrorDoc(doc())).toBe(0);
  });

  test("пустой параграф → 0 (не считаем сам блок если в нём ничего нет до него)", () => {
    expect(countCharsInProseMirrorDoc(doc(p()))).toBe(0);
  });

  test("один параграф с латиницей", () => {
    expect(countCharsInProseMirrorDoc(doc(p(t("hello"))))).toBe(5);
  });

  test("один параграф с кириллицей", () => {
    expect(countCharsInProseMirrorDoc(doc(p(t("привет"))))).toBe(6);
  });

  test("два параграфа разделены newline (Enter между ними считается)", () => {
    // hello\nworld визуально = 11 символов, не 10.
    expect(countCharsInProseMirrorDoc(doc(p(t("hello")), p(t("world"))))).toBe(11);
  });

  test("три параграфа → 2 newline между ними", () => {
    expect(
      countCharsInProseMirrorDoc(doc(p(t("a")), p(t("b")), p(t("c")))),
    ).toBe(5);
  });

  test("пустой параграф между двумя непустыми считается как 2 newline", () => {
    // a\n\nb = 4 символа.
    expect(countCharsInProseMirrorDoc(doc(p(t("a")), p(), p(t("b"))))).toBe(4);
  });

  test("hard_break (Shift+Enter) считается как один символ", () => {
    expect(
      countCharsInProseMirrorDoc(
        doc(p(t("a"), { type: "hardBreak" }, t("b"))),
      ),
    ).toBe(3);
  });

  test("hard_break в snake_case (резерв на случай нестандартной сериализации)", () => {
    expect(
      countCharsInProseMirrorDoc(
        doc(p(t("a"), { type: "hard_break" }, t("b"))),
      ),
    ).toBe(3);
  });

  test("emoji (surrogate pair) считается за 1 символ, а не за 2", () => {
    // 😀 = U+1F600 = две UTF-16 единицы. Naive .length вернул бы 2.
    expect(countCharsInProseMirrorDoc(doc(p(t("😀"))))).toBe(1);
    expect(countCharsInProseMirrorDoc(doc(p(t("hi 😀"))))).toBe(4);
  });

  test("несколько emoji подряд считаются по одному за каждое", () => {
    expect(countCharsInProseMirrorDoc(doc(p(t("👋🚀🔥"))))).toBe(3);
  });

  test("heading + paragraph: newline между ними тоже считается", () => {
    const json = doc(
      { type: "heading", attrs: { level: 1 }, content: [t("Заголовок")] },
      p(t("текст")),
    );
    // "Заголовок" (9) + \n (1) + "текст" (5) = 15.
    expect(countCharsInProseMirrorDoc(json)).toBe(15);
  });

  test("bullet list: каждый list_item с paragraph даёт свой newline", () => {
    const json = doc({
      type: "bulletList",
      content: [
        { type: "listItem", content: [p(t("first"))] },
        { type: "listItem", content: [p(t("second"))] },
      ],
    });
    // "first" + \n + "second" = 12. Container (bulletList, listItem)
    // сам по себе newline не вносит.
    expect(countCharsInProseMirrorDoc(json)).toBe(12);
  });

  test("blockquote: внутренний paragraph учитывается как leaf-block", () => {
    const json = doc(
      p(t("intro")),
      { type: "blockquote", content: [p(t("quoted"))] },
      p(t("outro")),
    );
    // "intro" + \n + "quoted" + \n + "outro" = 5+1+6+1+5 = 18.
    expect(countCharsInProseMirrorDoc(json)).toBe(18);
  });

  test("codeBlock считается как leaf-block", () => {
    const json = doc(
      p(t("before")),
      { type: "codeBlock", content: [t("const x = 1;")] },
      p(t("after")),
    );
    // 6 + 1 + 12 + 1 + 5 = 25.
    expect(countCharsInProseMirrorDoc(json)).toBe(25);
  });

  test("несколько text node'ов в одном параграфе (после mark split) суммируются без newline", () => {
    // Bold split: "hello world" = одна строка из двух text node'ов, marks
    // у второго. Между ними newline быть НЕ должно.
    const json = doc(
      p(t("hello "), { type: "text", text: "world", marks: [{ type: "bold" }] }),
    );
    expect(countCharsInProseMirrorDoc(json)).toBe(11);
  });

  test("countCharsInProseMirrorNode работает напрямую с объектом без JSON-парсинга", () => {
    const node = { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "raw" }] }] };
    expect(countCharsInProseMirrorNode(node)).toBe(3);
  });
});
