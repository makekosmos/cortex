// TaskRef — TipTap atomic block node, рендерит ссылку на task_obj в ARK как
// inline-checkbox с title. Source of truth — task_obj в ARK; content_json
// заметки хранит только `{type: "taskRef", attrs: {taskId}}`. Это Pattern B
// архитектуры (см. .agent/tasks/2026-05-20-eden-task-as-page/spec.md):
// объект — первичен, document — рендер.
//
// NodeView (TaskRefView.vue) подписывается на ARK `object_upserted`/`object_deleted`
// событие для своего taskId — bidir sync с Delphi работает live без любого
// diff/snapshot кода со стороны Editor.vue.

import { Node, mergeAttributes, nodeInputRule } from "@tiptap/core";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import { Plugin, PluginKey, TextSelection, AllSelection } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import TaskRefView from "./components/TaskRefView.vue";
import { edenApi } from "@/lib/edenApi";

const rangeSelectionPluginKey = new PluginKey("eden-task-ref-range-selection");

export interface TaskRefOptions {
  /**
   * Возвращает id текущей заметки. Используется input rule'ом
   * `[ ]` → TaskRef для propsJson.source_note_id нового task_obj.
   * Editor.vue прокидывает это через configure() из props.entry.id (с
   * reactive lookup'ом — заметка может смениться).
   */
  getSourceNoteId: () => string | null;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    taskRef: {
      /**
       * Вставить TaskRef node с уже созданным task_obj.id.
       * Создание task_obj — забота caller'а (`edenApi.createTask` вернёт id).
       */
      insertTaskRef: (taskId: string) => ReturnType;
    };
  }
}

export const TaskRef = Node.create<TaskRefOptions>({
  name: "taskRef",
  group: "block",
  atom: true,
  draggable: true,
  selectable: true,

  addOptions() {
    return {
      getSourceNoteId: () => null,
    };
  },

  addAttributes() {
    return {
      taskId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-task-id"),
        renderHTML: (attributes: { taskId?: string | null }) =>
          attributes.taskId ? { "data-task-id": attributes.taskId } : {},
      },
      // Runtime hint: «нода только что вставлена, NodeView должен сразу войти
      // в edit mode title». Не сериализуется в HTML — это эфемерный flag
      // только для свежесозданных задач (через slash или input rule). После
      // первого mount NodeView обнуляет его через updateAttributes.
      autoFocus: {
        default: false,
        parseHTML: () => false,
        renderHTML: () => ({}),
        keepOnSplit: false,
      },
    };
  },

  parseHTML() {
    return [{ tag: "div[data-task-ref]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-task-ref": "" })];
  },

  addNodeView() {
    return VueNodeViewRenderer(TaskRefView);
  },

  addCommands() {
    return {
      insertTaskRef:
        (taskId: string) =>
        ({ chain }) =>
          chain()
            .insertContent({
              type: this.name,
              attrs: { taskId, autoFocus: true },
            })
            .focus()
            .run(),
    };
  },

  addProseMirrorPlugins() {
    // Range-selection awareness: когда юзер тянет text selection через
    // несколько блоков и selection захватывает таск — навешиваем класс на
    // node DOM, чтобы CSS показал visual highlight. Это решает проблему
    // «atom block в range selection ничего не подсвечивает» — пользователь
    // не видит что taskRef включен, хотя ProseMirror selection технически
    // его держит. Anytype делает то же самое, но в их own block editor.
    return [
      new Plugin({
        key: rangeSelectionPluginKey,
        props: {
          decorations(state) {
            const sel = state.selection;
            // Empty selection (просто cursor) — никаких highlights.
            if (sel.empty) return null;
            // TextSelection / AllSelection включают atom-блоки в range
            // и должны их подсветить. NodeSelection — отдельный case
            // (ProseMirror уже даёт `.ProseMirror-selectednode` класс).
            if (!(sel instanceof TextSelection) && !(sel instanceof AllSelection)) {
              return null;
            }

            const { from, to } = sel;
            const decos: Decoration[] = [];
            state.doc.descendants((node, pos) => {
              if (node.type.name !== "taskRef") return;
              const nodeFrom = pos;
              const nodeTo = pos + node.nodeSize;
              // Node intersects selection range. Для атомарных block'ов
              // считаем включением если selection полностью покрывает
              // node (from <= nodeFrom && nodeTo <= to) ИЛИ selection
              // strictly intersects boundaries (стандартный case
              // когда юзер тянет курсор сверху или снизу через node).
              const fullyCovered = from <= nodeFrom && nodeTo <= to;
              const partialIntersect = nodeFrom < to && nodeTo > from;
              if (fullyCovered || partialIntersect) {
                decos.push(
                  Decoration.node(nodeFrom, nodeTo, {
                    class: "task-ref-range-selected",
                  }),
                );
              }
            });
            return decos.length ? DecorationSet.create(state.doc, decos) : null;
          },
        },
      }),
    ];
  },

  addInputRules() {
    const getSourceNoteId = this.options.getSourceNoteId;
    const type = this.type;
    return [
      // Markdown shortcut: `- [ ] ` или `[ ] ` в начале текста → TaskRef.
      // ProseMirror отрабатывает inputRule после ввода каждого char'а. После
      // того как BulletList (StarterKit) сожрёт `- ` → курсор в пустом
      // списочном элементе → юзер ввёл `[ ] ` → матчим, заменяем block на
      // TaskRef. Если пользователь набрал `- [ ]` слитно до того как
      // BulletList конвертировал `- ` — второй паттерн (`-\s\[\s\]\s`)
      // ловит его раньше.
      nodeInputRule({
        find: /^(?:-\s)?\[\s\]\s$/,
        type,
        getAttributes: () => {
          const taskId = crypto.randomUUID();
          const sourceNoteId = getSourceNoteId();
          if (sourceNoteId) {
            // Side-effect: async создаём task_obj с тем же taskId, который
            // вставляем синхронно в node. Race: NodeView mount'ится до того
            // как upsert завершится — loadTask вернёт null. NodeView сам
            // делает retry через 250ms (см. TaskRefView.vue).
            void edenApi.createTask(sourceNoteId, "", taskId);
          } else {
            console.warn("[eden TaskRef] input rule: sourceNoteId недоступен");
          }
          return { taskId, autoFocus: true };
        },
      }),
    ];
  },
});
