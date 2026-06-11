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

// Рекурсивно заменяет U+FF3B (fullwidth `[`) обратно на ASCII `[` в text-нодах
// ProseMirror JSON. Нужно чтобы content_json хранил корректные ASCII-чекбоксы.
function restoreCheckboxBracketsInJson(node: JSONContent): JSONContent {
  if (node.type === "text" && typeof node.text === "string") {
    return { ...node, text: node.text.replaceAll("［", "[") };
  }
  if (Array.isArray(node.content)) {
    return { ...node, content: node.content.map(restoreCheckboxBracketsInJson) };
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
      const raw = manager.serialize(doc as JSONContent);
      // Восстанавливаем `[` из fullwidth placeholder'а U+FF3B в чекбокс-строках.
      // Также убираем экранирование если serializer добавил `\[` перед `［`.
      return (
        raw
          .replace(/^(\s*[-*+]\s+)\\?［([x ])\]\\?/gm, "$1[$2]")
          // На случай если serializer экранировал ASCII `[` в начале listItem текста
          .replace(/^(\s*[-*+]\s+)\\\[([x ])\]/gm, "$1[$2]")
      );
    },

    markdownToJson(md: string): object {
      // Перед парсингом заменяем ASCII `[` → U+FF3B в чекбокс-строках.
      // U+FF3B не матчится listIsTask /^\[[ xX]\] +\S/ → нет task item →
      // listItem с текстом «［ ] текст» → serialize → «- ［ ] текст» →
      // postprocess jsonToMarkdown возвращает «- [ ] текст».
      const preprocessed = md.replace(CHECKBOX_PRE_RE, "$1［$2]");
      const parsed = manager.parse(preprocessed) as JSONContent;
      // Постпроцессинг: восстанавливаем ASCII-скобки в text-нодах,
      // чтобы content_json хранил «[ ]» / «[x]» вместо fullwidth-символов.
      return restoreCheckboxBracketsInJson(parsed);
    },

    destroy(): void {
      editor.destroy();
    },
  };
}
