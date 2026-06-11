// Портировано из ZenNotes (MIT, © 2026 Adib Hanna and ZenNotes contributors), адаптировано для Eden.
/**
 * Tags every line inside a fenced or indented code block with
 * `cm-code-block-line` so the stylesheet can render the whole block in the
 * configured monospace font (`--font-mono`).
 *
 * Without this, only inline code and the rendered preview use the mono font —
 * fenced code-block content in the editor inherits the body text font, which
 * is often proportional.
 */
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";

const codeBlockLine = Decoration.line({ class: "cm-code-block-line" });

function buildDecorations(view: EditorView): DecorationSet {
  const tree = syntaxTree(view.state);
  const lineStarts = new Set<number>();
  for (const { from, to } of view.visibleRanges) {
    tree.iterate({
      from,
      to,
      enter: (node) => {
        if (node.name !== "FencedCode" && node.name !== "CodeBlock") return;
        let pos = node.from;
        while (pos <= node.to) {
          const line = view.state.doc.lineAt(pos);
          lineStarts.add(line.from);
          if (line.to >= node.to) break;
          pos = line.to + 1;
        }
        return false;
      },
    });
  }
  const builder = new RangeSetBuilder<Decoration>();
  for (const from of [...lineStarts].sort((a, b) => a - b)) {
    builder.add(from, from, codeBlockLine);
  }
  return builder.finish();
}

export const codeBlockFontPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }

    update(update: ViewUpdate): void {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
