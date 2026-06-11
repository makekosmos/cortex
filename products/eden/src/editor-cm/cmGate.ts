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
]);

export const CM_SAFE_MARKS = new Set(["bold", "italic", "strike", "code", "link"]);

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

export function isCmSafeDoc(doc: unknown): boolean {
  return isNodeSafe(doc);
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
