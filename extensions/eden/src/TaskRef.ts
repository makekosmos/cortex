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
import { Plugin } from "@tiptap/pm/state";
import TaskRefView from "./components/TaskRefView.vue";
import { edenApi } from "@/lib/edenApi";

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

  // Range-selection awareness живёт внутри NodeView (TaskRefView.vue) —
  // каждый view подписан на `editor.on("selectionUpdate")` и сам решает
  // подсвечен ли через reactive ref. Это надёжнее чем ProseMirror
  // `Decoration.node`: Vue NodeView перерисовывает wrapper и теряет класс
  // декорации (Decoration.node ставит attr на outer DOM, но Vue им рулит).

  addProseMirrorPlugins() {
    return [
      new Plugin({
        props: {
          /**
           * Defense-in-depth: если PM получает keydown Enter когда фокус
           * на `<input class="task-ref-title-input">` (через любой обходной
           * путь — Vue handler не отработал, NodeView.stopEvent override
           * сломан, etc.), возвращаем true чтобы PM keymap chain
           * (createParagraphNear / splitBlock) не запускался. Vue handler
           * уже сделал свою работу — создал новую task, нам не нужны
           * параллельные PM operations.
           */
          handleKeyDown(_view, event) {
            if (event.key !== "Enter") return false;
            const target = event.target as HTMLElement | null;
            if (target?.classList?.contains("task-ref-title-input")) {
              return true;
            }
            return false;
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
