// TaskItemWithId — расширение @tiptap/extension-task-item с обязательным
// атрибутом `taskId` (UUID v4). Используется для связи taskItem ↔ task_obj
// в ARK: при сохранении заметки diff'аем список taskId'ов в content_json
// против предыдущего snapshot'а и создаём/обновляем/soft-deletе'м task_obj.
//
// taskId проставляется через ProseMirror plugin `appendTransaction` — на
// каждую транзакцию находим taskItem без taskId и проставляем UUID. Это
// покрывает все способы создания: `toggleTaskList` из slash-команды,
// enter-split, paste, drag-drop.
//
// Известное ограничение MVP: copy-paste taskItem'а сохраняет `taskId` копии
// → upsert_object идемпотентно обновит тот же task_obj. То есть копия = тот
// же task. Это намеренно — простая семантика «один visual checkbox = одна
// задача в Delphi». Альтернатива (regenerate при duplicate) — отдельный
// follow-up.

import TaskItem from "@tiptap/extension-task-item";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { v4 as uuidv4 } from "uuid";

const taskIdPluginKey = new PluginKey("eden-task-id");

export const TaskItemWithId = TaskItem.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      taskId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-task-id"),
        renderHTML: (attributes: { taskId?: string | null }) => {
          if (!attributes.taskId) return {};
          return { "data-task-id": attributes.taskId };
        },
        keepOnSplit: false,
      },
    };
  },

  addProseMirrorPlugins() {
    const typeName = this.name;
    return [
      new Plugin({
        key: taskIdPluginKey,
        appendTransaction(_transactions, _oldState, newState) {
          const tr = newState.tr;
          let mutated = false;
          newState.doc.descendants((node, pos) => {
            if (node.type.name !== typeName) return;
            if (node.attrs.taskId) return;
            tr.setNodeMarkup(pos, undefined, { ...node.attrs, taskId: uuidv4() });
            mutated = true;
          });
          return mutated ? tr : null;
        },
      }),
    ];
  },
});
