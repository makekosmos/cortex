// Gate: какие PM-документы безопасны для CM6 markdown-редактора (фаза 1).

export const CM_SAFE_NODES = new Set([
  "doc",
  "paragraph",
  "heading",
  "text",
  "bulletList",
  "orderedList",
  "listItem",
  "blockquote",
  "codeBlock",
  "hardBreak",
  "horizontalRule",
  // Legacy TipTap task nodes. CmEditor serializes them as plain markdown
  // checklist text before handing the document to the legacy markdown converter.
  "taskList",
  "taskItem",
]);

export const CM_SAFE_MARKS = new Set(["bold", "italic", "strike", "code", "link"]);

type JsonNode = Record<string, unknown>;

function textNode(text: string): JsonNode {
  return { type: "text", text };
}

function paragraphNode(text: string): JsonNode {
  const trimmed = text.trim();
  return trimmed ? { type: "paragraph", content: [textNode(trimmed)] } : { type: "paragraph" };
}

function sanitizeMarks(marks: unknown): unknown[] | undefined {
  if (!Array.isArray(marks)) return undefined;
  const safeMarks = marks.filter((mark): mark is JsonNode => {
    if (!mark || typeof mark !== "object" || Array.isArray(mark)) return false;
    return typeof mark.type === "string" && CM_SAFE_MARKS.has(mark.type);
  });
  return safeMarks.length > 0 ? safeMarks : undefined;
}

// Рекурсивно проверяет узел PM JSON на принадлежность к allowlist.
function isNodeSafe(node: unknown): boolean {
  if (!node || typeof node !== "object" || Array.isArray(node)) {
    return false;
  }

  const n = node as Record<string, unknown>;

  if (typeof n.type !== "string") {
    return false;
  }

  if (!CM_SAFE_NODES.has(n.type)) {
    return false;
  }

  // Проверяем марки текстовой ноды
  if (Array.isArray(n.marks)) {
    for (const mark of n.marks) {
      if (!mark || typeof mark !== "object" || Array.isArray(mark)) {
        return false;
      }
      const m = mark as Record<string, unknown>;
      if (typeof m.type !== "string" || !CM_SAFE_MARKS.has(m.type)) {
        return false;
      }
    }
  }

  // Рекурсивно проверяем дочерние ноды
  if (Array.isArray(n.content)) {
    for (const child of n.content) {
      if (!isNodeSafe(child)) {
        return false;
      }
    }
  }

  return true;
}

function collectCmBlockers(node: unknown, blockers: Set<string>): void {
  if (!node || typeof node !== "object" || Array.isArray(node)) {
    blockers.add("invalid-node");
    return;
  }

  const n = node as Record<string, unknown>;

  if (typeof n.type !== "string") {
    blockers.add("missing-node-type");
    return;
  }

  if (!CM_SAFE_NODES.has(n.type)) {
    blockers.add(`node:${n.type}`);
  }

  if (Array.isArray(n.marks)) {
    for (const mark of n.marks) {
      if (!mark || typeof mark !== "object" || Array.isArray(mark)) {
        blockers.add("invalid-mark");
        continue;
      }
      const m = mark as Record<string, unknown>;
      if (typeof m.type !== "string") {
        blockers.add("missing-mark-type");
      } else if (!CM_SAFE_MARKS.has(m.type)) {
        blockers.add(`mark:${m.type}`);
      }
    }
  }

  if (Array.isArray(n.content)) {
    for (const child of n.content) {
      collectCmBlockers(child, blockers);
    }
  }
}

function extractPlainText(node: unknown): string {
  if (!node || typeof node !== "object" || Array.isArray(node)) return "";
  const n = node as JsonNode;

  if (n.type === "text" && typeof n.text === "string") {
    return n.text;
  }

  const attrs = n.attrs && typeof n.attrs === "object" ? (n.attrs as JsonNode) : null;
  if (n.type === "wikilink" && typeof attrs?.target === "string") {
    return `[[${attrs.target}]]`;
  }
  if (n.type === "taskRef") {
    const title =
      typeof attrs?.titleSnapshot === "string"
        ? attrs.titleSnapshot
        : typeof attrs?.title === "string"
          ? attrs.title
          : "";
    return title.trim() ? `[ ] ${title.trim()}` : "[ ]";
  }

  if (!Array.isArray(n.content)) return "";
  return n.content.map(extractPlainText).filter(Boolean).join(" ");
}

function coerceNodeToCmSafe(node: unknown): JsonNode[] {
  if (!node || typeof node !== "object" || Array.isArray(node)) {
    return [];
  }

  const n = node as JsonNode;
  if (typeof n.type !== "string") {
    return [];
  }

  if (n.type === "text") {
    if (typeof n.text !== "string" || n.text.length === 0) return [];
    const marks = sanitizeMarks(n.marks);
    return marks ? [{ type: "text", text: n.text, marks }] : [textNode(n.text)];
  }

  if (n.type === "taskRef") {
    return [
      {
        type: "bulletList",
        content: [
          {
            type: "listItem",
            content: [paragraphNode(extractPlainText(n))],
          },
        ],
      },
    ];
  }

  if (n.type === "paragraph" || n.type === "heading") {
    const content = Array.isArray(n.content)
      ? n.content
          .map((child) => {
            const childNode = child as JsonNode;
            if (
              childNode &&
              typeof childNode === "object" &&
              !Array.isArray(childNode) &&
              childNode.type === "text" &&
              typeof childNode.text === "string"
            ) {
              const marks = sanitizeMarks(childNode.marks);
              return marks
                ? { type: "text", text: childNode.text, marks }
                : textNode(childNode.text);
            }

            const text = extractPlainText(child);
            return text ? textNode(text) : null;
          })
          .filter((child): child is JsonNode => child !== null)
      : [];

    const next: JsonNode = { type: n.type };
    if (n.type === "heading" && n.attrs && typeof n.attrs === "object" && !Array.isArray(n.attrs)) {
      next.attrs = n.attrs;
    }
    if (content.length > 0) next.content = content;
    return [next];
  }

  if (CM_SAFE_NODES.has(n.type)) {
    const next: JsonNode = { type: n.type };
    if (n.attrs && typeof n.attrs === "object" && !Array.isArray(n.attrs)) {
      next.attrs = n.attrs;
    }
    if (Array.isArray(n.content)) {
      const children = n.content.flatMap(coerceNodeToCmSafe);
      if (children.length > 0) next.content = children;
    }
    return [next];
  }

  const text = extractPlainText(n);
  return text.trim() ? [paragraphNode(text)] : [];
}

export function isCmSafeDoc(doc: unknown): boolean {
  return isNodeSafe(doc);
}

export function coerceToCmSafeDoc(doc: unknown): object {
  if (isCmSafeDoc(doc)) return doc as object;
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }

  const root = doc as JsonNode;
  const content = Array.isArray(root.content) ? root.content.flatMap(coerceNodeToCmSafe) : [];
  return {
    type: "doc",
    content: content.length > 0 ? content : [{ type: "paragraph" }],
  };
}

export function shouldUseCmEditor(prefEnabled: boolean, contentJson: string): boolean {
  if (!prefEnabled) return false;

  let parsed: unknown;
  try {
    parsed = JSON.parse(contentJson);
  } catch {
    return false;
  }

  return isCmSafeDoc(parsed);
}

export function getCmEditorBlockers(contentJson: string): string[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(contentJson);
  } catch {
    return ["invalid-json"];
  }

  const blockers = new Set<string>();
  collectCmBlockers(parsed, blockers);
  return [...blockers].sort();
}
