import { Node, mergeAttributes } from "@tiptap/vue-3";

import Suggestion from "@tiptap/suggestion";

import type { Editor } from "@tiptap/vue-3";

import { PluginKey } from "@tiptap/pm/state";

const wikilinkSuggestionPluginKey = new PluginKey("wikilinkSuggestion");

export const Wikilink = Node.create({
  name: "wikilink",

  group: "inline",

  inline: true,

  selectable: true,

  atom: true,

  addAttributes() {
    return {
      id: {
        default: null,

        parseHTML: (element) => element.getAttribute("data-id"),

        renderHTML: (attributes) => {
          if (!attributes.id) return {};

          return { "data-id": attributes.id };
        },
      },

      label: {
        default: null,

        parseHTML: (element) => element.getAttribute("data-label"),

        renderHTML: (attributes) => {
          if (!attributes.label) return {};

          return { "data-label": attributes.label };
        },
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-wikilink]" }];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      "span",

      mergeAttributes({ "data-wikilink": "" }, HTMLAttributes),

      `[[${node.attrs.label}]]`,
    ];
  },

  // @ts-expect-error — renderMarkdown registered via @tiptap/markdown
  renderMarkdown(node: { attrs?: { label?: string; id?: string } }): string {
    return `[[${node.attrs?.label ?? node.attrs?.id ?? ""}]]`;
  },

  addOptions() {
    return {
      suggestion: {
        char: "[[",

        command: ({
          editor,

          range,

          props,
        }: {
          editor: Editor;

          range: { from: number; to: number };

          props: Record<string, unknown>;
        }) => {
          editor

            .chain()

            .focus()

            .deleteRange(range)

            .insertContent([
              { type: this.name, attrs: props },

              { type: "text", text: " " },
            ])

            .run();
        },
      },
    };
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,

        pluginKey: wikilinkSuggestionPluginKey,

        ...this.options.suggestion,
      }),
    ];
  },
});
