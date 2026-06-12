// Markdown ↔ ProseMirror JSON converter для CM6 editor.
// Использует headless TipTap v3 Editor с @tiptap/markdown.
//
// API v3:
//   - editor.storage.markdown.manager.serialize(json) → string
//   - editor.storage.markdown.manager.parse(markdown) → JSONContent

import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import type { JSONContent } from "@tiptap/core";

export interface MdConverter {
  jsonToMarkdown(doc: object): string;
  markdownToJson(md: string): object;
  destroy(): void;
}

// Препроцессинг чекбоксов перед парсингом markdown:
// `- [ ]` / `- [x]` в начале строки заменяем `[` на U+FF3B (fullwidth bracket).
// Паттерн listIsTask в marked v17: /^\[[ xX]\] +\S/ — матчит только ASCII `[`.
// U+FF3B не матчится, поэтому item парсится как обычный listItem.
// Постпроцессинг в jsonToMarkdown восстанавливает `[` обратно.
const CHECKBOX_PRE_RE = /^(\s*[-*+]\s+)\[([x ])\]/gim;
const IMAGE_PRE_RE = /!\[/g;

function textNode(text: string): JSONContent {
  return { type: "text", text };
}

function paragraph(content: JSONContent[]): JSONContent {
  return { type: "paragraph", content };
}

function listItem(content: JSONContent[]): JSONContent {
  return { type: "listItem", content };
}

function normalizeLegacyTaskNodes(node: JSONContent): JSONContent {
  if (node.type === "taskList") {
    return {
      type: "bulletList",
      content: (node.content ?? []).map(normalizeLegacyTaskNodes),
    };
  }

  if (node.type === "taskItem") {
    const checked = (node.attrs as { checked?: boolean } | undefined)?.checked === true;
    const normalizedChildren = (node.content ?? []).map(normalizeLegacyTaskNodes);
    const marker = textNode(checked ? "[x] " : "[ ] ");

    if (normalizedChildren.length === 0) {
      return listItem([paragraph([marker])]);
    }

    const [firstChild, ...restChildren] = normalizedChildren;
    if (firstChild.type === "paragraph") {
      return listItem([
        { ...firstChild, content: [marker, ...(firstChild.content ?? [])] },
        ...restChildren,
      ]);
    }

    return listItem([paragraph([marker]), firstChild, ...restChildren]);
  }

  if (Array.isArray(node.content)) {
    return { ...node, content: node.content.map(normalizeLegacyTaskNodes) };
  }

  return node;
}

// Рекурсивно заменяет временные fullwidth markers обратно на ASCII в text-нодах
// ProseMirror JSON. Нужно чтобы content_json хранил корректный Markdown.
function restoreMarkdownPlaceholdersInJson(node: JSONContent): JSONContent {
  if (node.type === "text" && typeof node.text === "string") {
    return { ...node, text: node.text.replaceAll("［", "[").replaceAll("！[", "![") };
  }
  if (Array.isArray(node.content)) {
    return { ...node, content: node.content.map(restoreMarkdownPlaceholdersInJson) };
  }
  return node;
}

export function createMdConverter(): MdConverter {
  // Headless editor — элемент вне DOM, не отображается пользователю.
  // trailingNode: false — убирает trailing paragraph чтобы round-trip был стабильным.
  const el = document.createElement("div");

  const editor = new Editor({
    element: el,
    extensions: [StarterKit.configure({ trailingNode: false }), Markdown],
    editable: false,
  });

  const manager = editor.storage.markdown.manager;

  return {
    jsonToMarkdown(doc: object): string {
      // Прямая сериализация через MarkdownManager — не меняет состояние редактора.
      const normalizedDoc = normalizeLegacyTaskNodes(doc as JSONContent);
      const raw = manager.serialize(normalizedDoc);
      // Восстанавливаем `[` из fullwidth placeholder'а U+FF3B в чекбокс-строках.
      // Также убираем экранирование если serializer добавил `\[` перед `［`.
      return (
        raw
          .replace(/^(\s*[-*+]\s+)\\?［([x ])\]\\?/gm, "$1[$2]")
          // На случай если serializer экранировал ASCII `[` в начале listItem текста
          .replace(/^(\s*[-*+]\s+)\\\[([x ])\]/gm, "$1[$2]")
          // Image markdown хранится как plain text через fullwidth placeholder.
          .replaceAll("！[", "![")
          .replaceAll("\\![", "![")
      );
    },

    markdownToJson(md: string): object {
      // Перед парсингом заменяем ASCII `[` → U+FF3B в чекбокс-строках.
      // U+FF3B не матчится listIsTask /^\[[ xX]\] +\S/ → нет task item →
      // listItem с текстом «［ ] текст» → serialize → «- ［ ] текст» →
      // postprocess jsonToMarkdown возвращает «- [ ] текст».
      const preprocessed = md.replace(CHECKBOX_PRE_RE, "$1［$2]").replace(IMAGE_PRE_RE, "！[");
      const parsed = manager.parse(preprocessed) as JSONContent;
      // Постпроцессинг: восстанавливаем ASCII markers в text-нодах,
      // чтобы content_json хранил обычный Markdown вместо fullwidth-символов.
      return restoreMarkdownPlaceholdersInJson(parsed);
    },

    destroy(): void {
      editor.destroy();
    },
  };
}
