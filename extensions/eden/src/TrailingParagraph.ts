import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

/**
 * TrailingParagraph — гарантирует, что последний top-level node документа —
 * пустой paragraph. Если он там уже есть — no-op (поэтому safe для
 * idempotent dispatch'ев).
 *
 * Почему не `@tiptap/extension-trailing-node` из StarterKit:
 *   StarterKit'овский TrailingNode добавляет paragraph если последний child
 *   — НЕ textblock (наш `taskRef` — atom, не textblock). Это конфликтовало
 *   с TaskRef.commitAndCreateNew который вручную удаляет / переиспользует
 *   trailing paragraph (см. docs-site/concepts/eden-taskref-pm-traps.md
 *   ловушка #2). Когда `trailingNode: false`, comment в commitAndCreateNew
 *   («paragraph PM добавит обратно автоматически») перестал работать —
 *   PM не добавляет.
 *
 *   Этот plugin делает ровно то, что комментарий ожидал: после ЛЮБОЙ
 *   transaction'и проверяем последний node. Если это уже пустой paragraph
 *   — ничего не делаем. Если нет (или таблица, или taskRef, или непустой
 *   paragraph) — append пустой paragraph через `tr.insert(doc.content.size,
 *   para)`.
 *
 *   Поскольку appendTransaction видит результат `commitAndCreateNew`
 *   ПОСЛЕ его `tr.insert(insertPos, newTaskRef)` — мы попадаем в ветку
 *   «последний node = новый taskRef (atom)» → добавляем пустой paragraph.
 *   Никакого дублирования (там не было empty paragraph после `deleteTail`)
 *   и никакого «висящего» paragraph после произвольного Enter внутри
 *   taskRef (Enter в TaskRefView делает commitAndCreateNew/exitToNewParagraph,
 *   а не PM-native split — поэтому intermediate "лишний" paragraph
 *   не возникает).
 *
 *   Для TaskRef trap #2: trap описывал ситуацию когда `tr.replaceWith` /
 *   `tr.delete`+`tr.insert` отрабатывали корректно, а потом TrailingNode
 *   через appendTransaction ВЕЗДЕ где последний node — atom, добавлял
 *   paragraph. Наш plugin делает то же самое, НО commitAndCreateNew
 *   именно на это и рассчитывает (line 513 comment). Так что trap #2
 *   касался поведения, которое стало корректным after-fix.
 */
const trailingParagraphPluginKey = new PluginKey("edenTrailingParagraph");

export const TrailingParagraph = Extension.create({
  name: "trailingParagraph",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: trailingParagraphPluginKey,
        appendTransaction: (_transactions, _oldState, newState) => {
          const { doc, schema, tr } = newState;
          const paragraphType = schema.nodes.paragraph;
          if (!paragraphType) return null;

          const lastChild = doc.lastChild;
          // Последний node = пустой paragraph — ничего делать не надо.
          if (lastChild && lastChild.type === paragraphType && lastChild.content.size === 0) {
            return null;
          }

          // Иначе — append пустой paragraph в самый конец документа.
          // setMeta addToHistory=false: trailing paragraph — это invariant
          // doc'а, не пользовательское действие. Не должен попадать в
          // undo stack отдельным шагом.
          return tr.insert(doc.content.size, paragraphType.create()).setMeta("addToHistory", false);
        },
      }),
    ];
  },
});
