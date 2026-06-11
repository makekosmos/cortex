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
  // checklist text before handing the document to @tiptap/markdown.
  "taskList",
  "taskItem",
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
