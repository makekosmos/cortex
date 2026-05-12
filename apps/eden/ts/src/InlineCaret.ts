import { Extension } from "@tiptap/vue-3";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

const inlineCaretPluginKey = new PluginKey<{
  focused: boolean;
  composing: boolean;
}>("inlineCaret");

function createCaretAnchor() {
  const anchor = document.createElement("span");
  anchor.className = "pm-inline-caret-anchor";
  anchor.setAttribute("aria-hidden", "true");
  anchor.setAttribute("contenteditable", "false");
  return anchor;
}

export const InlineCaret = Extension.create({
  name: "inlineCaret",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: inlineCaretPluginKey,
        state: {
          init: () => ({
            focused: false,
            composing: false,
          }),
          apply(transaction, pluginState) {
            const meta = transaction.getMeta(inlineCaretPluginKey) as
              | Partial<typeof pluginState>
              | undefined;

            if (!meta) return pluginState;

            return {
              ...pluginState,
              ...meta,
            };
          },
        },
        props: {
          decorations: (state) => {
            const pluginState = inlineCaretPluginKey.getState(state);

            if (
              !pluginState?.focused ||
              pluginState.composing ||
              !state.selection.empty
            ) {
              return DecorationSet.empty;
            }

            return DecorationSet.create(state.doc, [
              Decoration.widget(state.selection.from, createCaretAnchor, {
                key: "pm-inline-caret",
                side: -1,
                ignoreSelection: true,
              }),
            ]);
          },
          handleDOMEvents: {
            focus: (view) => {
              view.dispatch(
                view.state.tr.setMeta(inlineCaretPluginKey, { focused: true }),
              );
              view.dom.classList.add("pm-inline-caret-enabled");
              return false;
            },
            blur: (view) => {
              view.dispatch(
                view.state.tr.setMeta(inlineCaretPluginKey, { focused: false }),
              );
              view.dom.classList.remove("pm-inline-caret-enabled");
              view.dom.classList.remove("pm-inline-caret-composing");
              return false;
            },
            compositionstart: (view) => {
              view.dispatch(
                view.state.tr.setMeta(inlineCaretPluginKey, { composing: true }),
              );
              view.dom.classList.remove("pm-inline-caret-enabled");
              view.dom.classList.add("pm-inline-caret-composing");
              return false;
            },
            compositionend: (view) => {
              view.dispatch(
                view.state.tr.setMeta(inlineCaretPluginKey, { composing: false }),
              );
              view.dom.classList.remove("pm-inline-caret-composing");
              view.dom.classList.add("pm-inline-caret-enabled");
              return false;
            },
          },
        },
        view: (view) => {
          if (view.hasFocus()) {
            view.dispatch(
              view.state.tr.setMeta(inlineCaretPluginKey, { focused: true }),
            );
            view.dom.classList.add("pm-inline-caret-enabled");
          }

          return {
            destroy: () => {
              view.dom.classList.remove("pm-inline-caret-enabled");
              view.dom.classList.remove("pm-inline-caret-composing");
            },
          };
        },
      }),
    ];
  },
});
