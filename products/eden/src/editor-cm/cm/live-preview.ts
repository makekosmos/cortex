// Портировано из ZenNotes (MIT, © 2026 Adib Hanna and ZenNotes contributors), адаптировано для Eden.
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, StateEffect } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";

/**
 * Live-preview extension: hides markdown syntax markers on lines where
 * the cursor (or any part of the selection) does not currently live.
 *
 * Obsidian-style WYSIWYG feel. When you move off a line the `#`, `**`,
 * `[`, `](url)`, backticks, etc. fade away and the heading/bold/link
 * renders cleanly. When you land on that line again, the markers come
 * back so you can edit them.
 *
 * Упрощено против оригинала: удалены LocalImageWidget, LocalPdfWidget,
 * все зависимости useStore/zustand, local-assets, image-block-dnd,
 * asset-tabs, expandedPdfsByView, pinned-ref логика. Оставлены только
 * скрытие синтаксиса и интерактивные чекбоксы TaskCheckboxWidget.
 */

/** Node names from @lezer/markdown that correspond to syntax markers. */
const SIMPLE_HIDE = new Set([
  "EmphasisMark",
  "CodeMark",
  "LinkMark",
  "StrikethroughMark",
  "CodeInfo",
]);

/** The `[ ]` or `[x]` marker inside a GFM task list item. Replaced by an
 *  interactive checkbox widget — see `TaskCheckboxWidget` below. */
const TASK_MARKER_NODE = "TaskMarker";

/** URL nodes need special handling: only hide when they are a link
 *  target `(url)`, not when they are autolinked text or appear inside
 *  a link label `[url](...)`. */
const URL_NODE = "URL";

/** Marks that typically have a trailing space we also want to hide. */
const PREFIX_HIDE_WITH_SPACE = new Set(["HeaderMark", "QuoteMark"]);

const hide = Decoration.replace({});

type PendingDecoration = {
  from: number;
  to: number;
  deco: Decoration;
};

type SyntaxNodeLike = {
  name: string;
  from: number;
  to: number;
  parent: SyntaxNodeLike | null;
};

type SyntaxNodeRefLike = {
  node: SyntaxNodeLike;
};

function selectionTouchesRange(state: EditorView["state"], from: number, to: number): boolean {
  for (const range of state.selection.ranges) {
    if (range.empty) {
      if (range.from >= from && range.from <= to) return true;
      continue;
    }
    if (Math.max(range.from, from) < Math.min(range.to, to)) return true;
  }
  return false;
}

function enclosingLinkRange(ref: SyntaxNodeRefLike): { from: number; to: number } | null {
  let node: SyntaxNodeLike | null = ref.node;
  while (node) {
    if (node.name === "Link" || node.name === "Image") {
      return { from: node.from, to: node.to };
    }
    if (node.name === "Paragraph" || node.name === "Document") break;
    node = node.parent;
  }
  return null;
}

/** Renders a GFM task-list marker (`[ ]` / `[x]` / `[X]`) as a clickable
 *  checkbox. The widget rewrites the underlying markdown when toggled. */
class TaskCheckboxWidget extends WidgetType {
  constructor(
    /** Absolute doc offset of the opening `[`. The marker is always 3
     *  chars (`[ ]`, `[x]`, `[X]`), so the inner state char is at `from + 1`. */
    private readonly from: number,
    private readonly checked: boolean,
  ) {
    super();
  }

  eq(other: TaskCheckboxWidget): boolean {
    return other.from === this.from && other.checked === this.checked;
  }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement("span");
    wrap.className = "cm-task-checkbox";
    wrap.setAttribute("contenteditable", "false");

    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = this.checked;
    input.className = "cm-task-checkbox-input";
    input.setAttribute("aria-label", this.checked ? "Uncheck task" : "Check task");

    // Stop the editor from moving the selection or losing focus on the
    // pointer-down phase.
    input.addEventListener("mousedown", (event) => {
      event.preventDefault();
      event.stopPropagation();
    });
    input.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const stateFrom = this.from + 1;
      const stateTo = this.from + 2;
      view.dispatch({
        changes: { from: stateFrom, to: stateTo, insert: this.checked ? " " : "x" },
      });
    });

    wrap.append(input);
    return wrap;
  }

  ignoreEvent(): boolean {
    return false;
  }
}

function computeDecorations(view: EditorView): DecorationSet {
  const { state } = view;

  const activeLines = new Set<number>();
  for (const r of state.selection.ranges) {
    const fromLine = state.doc.lineAt(r.from).number;
    const toLine = state.doc.lineAt(r.to).number;
    for (let l = fromLine; l <= toLine; l++) activeLines.add(l);
  }

  const pending: PendingDecoration[] = [];
  const replacedLines = new Set<number>();

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter: (node) => {
        const name = node.name;
        const isPrefix = PREFIX_HIDE_WITH_SPACE.has(name);
        const isSimple = SIMPLE_HIDE.has(name);
        const isUrl = name === URL_NODE;
        const isLinkSyntax = name === "LinkMark" || isUrl;

        if (name === TASK_MARKER_NODE) {
          const line = state.doc.lineAt(node.from).number;
          if (replacedLines.has(line)) return;
          if (selectionTouchesRange(state, node.from, node.to)) return;
          const markerText = state.doc.sliceString(node.from, node.to);
          const checked = markerText.length >= 2 && /[xX]/.test(markerText[1] ?? "");
          pending.push({
            from: node.from,
            to: node.to,
            deco: Decoration.replace({
              widget: new TaskCheckboxWidget(node.from, checked),
            }),
          });
          return;
        }

        // Only hide URL nodes that are link targets — preceded by `(`
        if (isUrl) {
          const prevChar = state.doc.sliceString(node.from - 1, node.from);
          if (prevChar !== "(") return;
        }

        if (!isPrefix && !isSimple && !isUrl) return;

        // Don't hide fenced code block delimiters (```) or language tags —
        // only hide inline code backticks.
        if ((name === "CodeMark" || name === "CodeInfo") && node.node.parent?.name === "FencedCode")
          return;

        const line = state.doc.lineAt(node.from).number;
        if (replacedLines.has(line)) return;
        if (isLinkSyntax) {
          const linkRange = enclosingLinkRange(node);
          if (linkRange && selectionTouchesRange(state, linkRange.from, linkRange.to)) return;
        } else if (activeLines.has(line)) {
          const keepHeadingMarkerHidden =
            name === "HeaderMark" && !selectionTouchesRange(state, node.from, node.to);
          if (!keepHeadingMarkerHidden) return;
        }

        let start = node.from;
        let end = node.to;
        if (end === start) return;

        if (isPrefix) {
          const next = state.doc.sliceString(end, end + 1);
          if (next === " " || next === "\t") end += 1;
        }

        pending.push({ from: start, to: end, deco: hide });
      },
    });
  }

  pending.sort((a, b) => {
    if (a.from !== b.from) return a.from - b.from;
    if (a.to !== b.to) return a.to - b.to;
    return 0;
  });

  const builder = new RangeSetBuilder<Decoration>();
  for (const item of pending) {
    builder.add(item.from, item.to, item.deco);
  }
  return builder.finish();
}

/** Dispatched when an external state change should force live-preview to
 *  recompute its decorations. */
export const refreshLivePreviewEffect = StateEffect.define<null>();

export const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = computeDecorations(view);
    }

    update(update: ViewUpdate): void {
      const externalRefresh = update.transactions.some((tr) =>
        tr.effects.some((e) => e.is(refreshLivePreviewEffect)),
      );
      if (
        update.docChanged ||
        update.selectionSet ||
        update.viewportChanged ||
        update.focusChanged ||
        externalRefresh
      ) {
        this.decorations = computeDecorations(update.view);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
);
