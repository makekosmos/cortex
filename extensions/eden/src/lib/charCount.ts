// Подсчёт символов в ProseMirror doc для bottom-center counter в zen mode.
//
// Считаем:
// - текст внутри `text` node'ов в code points (Array.from), а не UTF-16 code
//   units — иначе emoji 😀 даёт 2 вместо 1.
// - переносы между leaf-block'ами (paragraph / heading / codeBlock) как один
//   символ — пользователь нажимал Enter, символ виден.
// - `hardBreak` (Shift+Enter) как один символ.
//
// Containers (bulletList, orderedList, listItem, blockquote, doc) сами по
// себе newline не вносят — newline возникает у их leaf-block потомков.

const LEAF_BLOCK_TYPES = new Set<string>([
  "paragraph",
  "heading",
  "codeBlock",
  "code_block",
  "horizontalRule",
  "horizontal_rule",
]);

const HARD_BREAK_TYPES = new Set<string>(["hardBreak", "hard_break"]);

interface ProseMirrorNode {
  type?: string;
  text?: string;
  content?: unknown[];
}

export function countCharsInProseMirrorDoc(
  json: string | null | undefined,
): number | null {
  if (!json) return null;
  let doc: unknown;
  try {
    doc = JSON.parse(json);
  } catch {
    return null;
  }
  return countCharsInProseMirrorNode(doc);
}

export function countCharsInProseMirrorNode(doc: unknown): number {
  let total = 0;
  let leafBlocksSeen = 0;

  const walk = (node: unknown): void => {
    if (!node || typeof node !== "object") return;
    const n = node as ProseMirrorNode;

    if (n.type === "text" && typeof n.text === "string") {
      total += [...n.text].length;
      return;
    }
    if (typeof n.type === "string" && HARD_BREAK_TYPES.has(n.type)) {
      total += 1;
      return;
    }
    if (typeof n.type === "string" && LEAF_BLOCK_TYPES.has(n.type)) {
      if (leafBlocksSeen > 0) total += 1;
      leafBlocksSeen += 1;
    }
    if (Array.isArray(n.content)) {
      for (const child of n.content) walk(child);
    }
  };

  walk(doc);
  return total;
}
