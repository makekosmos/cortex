// BlockSelectionDecoration — TipTap extension с ProseMirror plugin'ом
// который держит DecorationSet с классом `.kepler-block-selected` на
// выделенных через rubber-band блоках.
//
// Почему не direct DOM mutation: PM перерендеривает DOM на каждый
// tr.dispatch (например при `editor.commands.blur()`), и imperative
// classList.add() стираются. Decoration API — PM-managed: декорации
// автоматически переапплятся после re-render'а, классы остаются.
//
// State plugin'а обновляется через `tr.setMeta(pluginKey, decorations)`
// из Editor.vue watcher'а на `selectedPositions`.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export const blockSelectionPluginKey = new PluginKey<DecorationSet>("eden-block-selection-decoration");

export const BlockSelectionDecoration = Extension.create({
  name: "blockSelectionDecoration",

  addProseMirrorPlugins() {
    return [
      new Plugin<DecorationSet>({
        key: blockSelectionPluginKey,
        state: {
          init: () => DecorationSet.empty,
          apply: (tr, prevDecoSet) => {
            // Caller сетит новый DecorationSet через meta.
            const meta = tr.getMeta(blockSelectionPluginKey);
            if (meta instanceof DecorationSet) {
              return meta;
            }
            // На любую другую транзакцию — мапим существующие декорации
            // на новый doc (positions могут сдвинуться при typing).
            if (tr.docChanged) {
              return prevDecoSet.map(tr.mapping, tr.doc);
            }
            return prevDecoSet;
          },
        },
        props: {
          decorations(state) {
            return blockSelectionPluginKey.getState(state) ?? DecorationSet.empty;
          },
        },
      }),
    ];
  },
});

/**
 * Helper: построить DecorationSet из множества block-positions. Каждая
 * position должна указывать на начало блока (то что doc.descendants даёт
 * как pos). Декорация — node-level с классом `.kepler-block-selected`.
 */
export function buildBlockSelectionDecorations(
  doc: import("@tiptap/pm/model").Node,
  selectedPositions: ReadonlySet<number>,
): DecorationSet {
  if (selectedPositions.size === 0) return DecorationSet.empty;
  const decos: Decoration[] = [];
  for (const pos of selectedPositions) {
    const node = doc.nodeAt(pos);
    if (!node) continue;
    decos.push(
      Decoration.node(pos, pos + node.nodeSize, {
        class: "kepler-block-selected",
      }),
    );
  }
  return DecorationSet.create(doc, decos);
}
