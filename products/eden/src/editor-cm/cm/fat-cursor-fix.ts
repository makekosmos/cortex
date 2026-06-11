import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

function restoreStyleProperty(
  el: HTMLElement,
  key: "width" | "min-width" | "height" | "max-height" | "line-height" | "transform",
  value: string,
): void {
  if (value) el.style.setProperty(key, value);
  else el.style.removeProperty(key);
}

function measureNaturalCursorRect(el: HTMLElement): DOMRect | null {
  const prevWidth = el.style.getPropertyValue("width");
  const prevMinWidth = el.style.getPropertyValue("min-width");
  const prevHeight = el.style.getPropertyValue("height");
  const prevMaxHeight = el.style.getPropertyValue("max-height");
  const prevLineHeight = el.style.getPropertyValue("line-height");
  const prevTransform = el.style.getPropertyValue("transform");

  el.style.removeProperty("width");
  el.style.removeProperty("min-width");
  el.style.height = "auto";
  el.style.maxHeight = "none";
  el.style.lineHeight = "normal";
  el.style.removeProperty("transform");

  const rect = el.getBoundingClientRect();

  restoreStyleProperty(el, "width", prevWidth);
  restoreStyleProperty(el, "min-width", prevMinWidth);
  restoreStyleProperty(el, "height", prevHeight);
  restoreStyleProperty(el, "max-height", prevMaxHeight);
  restoreStyleProperty(el, "line-height", prevLineHeight);
  restoreStyleProperty(el, "transform", prevTransform);

  return rect.width > 0 && rect.height > 0 ? rect : null;
}

function fixFatCursorHeight(view: EditorView): void {
  const cursors = view.scrollDOM.querySelectorAll<HTMLElement>(".cm-fat-cursor");
  for (const el of cursors) {
    const pluginHeight = Number.parseFloat(el.style.height);
    const naturalCursorRect = measureNaturalCursorRect(el);
    const naturalHeight = naturalCursorRect?.height ?? null;
    const targetHeight =
      naturalHeight && naturalHeight > 0
        ? Number.isFinite(pluginHeight) && pluginHeight > 0
          ? Math.min(pluginHeight, naturalHeight)
          : naturalHeight
        : Number.isFinite(pluginHeight) && pluginHeight > 0
          ? pluginHeight
          : null;
    if (!(targetHeight && targetHeight > 0)) continue;

    const targetWidth =
      naturalCursorRect?.width && naturalCursorRect.width > 0 ? naturalCursorRect.width : null;
    if (targetWidth) {
      el.style.width = `${targetWidth}px`;
      el.style.minWidth = `${targetWidth}px`;
    }
    el.style.height = `${targetHeight}px`;
    el.style.maxHeight = `${targetHeight}px`;
    el.style.lineHeight = "normal";
    el.style.removeProperty("transform");
  }
}

export const fatCursorFixPlugin = ViewPlugin.fromClass(
  class {
    private readonly view: EditorView;
    private cursorFixFrame = 0;

    constructor(view: EditorView) {
      this.view = view;
      this.cursorFixFrame = requestAnimationFrame(() => fixFatCursorHeight(this.view));
    }

    update(update: ViewUpdate): void {
      if (
        update.selectionSet ||
        update.geometryChanged ||
        update.docChanged ||
        update.viewportChanged
      ) {
        cancelAnimationFrame(this.cursorFixFrame);
        this.cursorFixFrame = requestAnimationFrame(() => fixFatCursorHeight(this.view));
      }
    }

    destroy(): void {
      cancelAnimationFrame(this.cursorFixFrame);
    }
  },
);
