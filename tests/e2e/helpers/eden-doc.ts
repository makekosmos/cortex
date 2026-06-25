// TipTap doc JSON builders для Eden e2e тестов.
//
// Чтобы не дублировать `{ type: "paragraph", content: [...] }` в каждом spec'е.

export interface PMNode {
  type: string;
  attrs?: Record<string, unknown>;
  content?: PMNode[];
  text?: string;
  marks?: Array<{ type: string; attrs?: Record<string, unknown> }>;
}

export interface PMDoc {
  type: "doc";
  content: PMNode[];
}

/** Пустой документ: один пустой paragraph. */
export function emptyDoc(): PMDoc {
  return { type: "doc", content: [paragraph()] };
}

/** Документ с N пустыми paragraph'ами (для теста дедупа trailing). */
export function emptyParagraphsDoc(n: number): PMDoc {
  return {
    type: "doc",
    content: Array.from({ length: n }, () => paragraph()),
  };
}

/** Paragraph с inline текстом (или пустой если text === undefined). */
export function paragraph(text?: string): PMNode {
  if (text == null) return { type: "paragraph" };
  return {
    type: "paragraph",
    content: [{ type: "text", text }],
  };
}

/** TipTap taskRef atom node. autoFocus=false по умолчанию — не хотим
 *  что входной фокус прыгнет при mount. */
export function taskRef(taskId: string, autoFocus = false): PMNode {
  return {
    type: "taskRef",
    attrs: { taskId, autoFocus },
  };
}

/** Heading node (level 1-6). */
export function heading(level: number, text: string): PMNode {
  return {
    type: "heading",
    attrs: { level },
    content: [{ type: "text", text }],
  };
}
