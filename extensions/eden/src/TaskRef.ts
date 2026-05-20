// TaskRef — TipTap atomic block node, рендерит ссылку на task_obj в ARK как
// inline-checkbox с title. Source of truth — task_obj в ARK; content_json
// заметки хранит только `{type: "taskRef", attrs: {taskId}}`. Это Pattern B
// архитектуры (см. .agent/tasks/2026-05-20-eden-task-as-page/spec.md):
// объект — первичен, document — рендер.
//
// NodeView (TaskRefView.vue) подписывается на ARK `object_upserted`/`object_deleted`
// событие для своего taskId — bidir sync с Delphi работает live без любого
// diff/snapshot кода со стороны Editor.vue.

import { Node, mergeAttributes } from "@tiptap/core";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import TaskRefView from "./components/TaskRefView.vue";

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

export const TaskRef = Node.create({
  name: "taskRef",
  group: "block",
  atom: true,
  draggable: true,
  selectable: true,

  addAttributes() {
    return {
      taskId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-task-id"),
        renderHTML: (attributes: { taskId?: string | null }) =>
          attributes.taskId ? { "data-task-id": attributes.taskId } : {},
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
              attrs: { taskId },
            })
            .focus()
            .run(),
    };
  },
});
