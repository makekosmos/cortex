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
  // draggable: false — PM при mousedown на draggable+atom выставляет
  // `target.draggable = true` через mightDrag flow, ломая нативное
  // поведение `<input>` (focus/caret). Drag-and-drop задач между
  // блоками сейчас не используется, отключение безопасно.
  draggable: false,
  selectable: false,

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

  // Markdown serialization для clipboard (Ctrl+C / Ctrl+X / drag-select).
  // `titleSnapshot` — runtime-only attr, патчится в Editor.vue из DOM input'а
  // перед serialize'ом (source of truth — ARK task_obj, не node attrs).
  // Поле читается @tiptap/markdown'ом через getExtensionField — типа в core
  // нет, поэтому ts-ignore.
  // @ts-expect-error — renderMarkdown registered via @tiptap/markdown MarkdownManager
  renderMarkdown(node: { attrs?: { titleSnapshot?: string } }): string {
    const title = (node.attrs?.titleSnapshot ?? "").trim();
    return `- [ ] ${title}`;
  },

  addNodeView() {
    return VueNodeViewRenderer(TaskRefView, {
      /**
       * PM's DOMObserver слушает document.selectionchange. Когда фокус
       * переходит на native `<input>` внутри NodeView, browser сбрасывает
       * doc selection → selectionchange fires → PM.onSelectionChange →
       * flush → updateSelection → view.focus() → focus уезжает с input
       * на .ProseMirror DIV.
       *
       * `ignoreMutation({type: "selection"})` говорит PM: «selection-
       * mutation'ы внутри этой NodeView нас не касаются». PM пропускает
       * `flush()` для них, и focus остаётся на input. Также ignor'им
       * атрибуты/childList — атомный NodeView рулит своим DOM сам, PM
       * не должен пересобирать его из model'и.
       */
      ignoreMutation: ({ mutation }) => {
        const type = (mutation as MutationRecord & { type: string }).type;
        if (type === "selection") return true;
        if (type === "attributes") return true;
        if (type === "childList") return true;
        if (type === "characterData") return true;
        return false;
      },
    });
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
          /**
           * Перехватываем PM click-обработку для taskRef нод когда клик
           * пришёлся на native UI внутри (input title / status / open).
           * PM по умолчанию на mouseup вызывает `selectClickedLeaf` →
           * `NodeSelection.create` → focus уезжает на .ProseMirror DIV
           * (PM собирает focus на view.dom когда селекция меняется),
           * и каретка теряется из нашего `<input>`.
           *
           * Возвращая true от handleClickOn — `handleSingleClick` PM'а
           * считает что мы обработали клик, не зовёт `selectClickedLeaf`,
           * вызывает `event.preventDefault()` на mouseup. Browser default
           * мaжhouseDOWN при этом уже сработал (focus input + caret-at-click),
           * а mouseUP-preventDefault это не отменяет.
           *
           * Также: handleDOMEvents.focus возвращаем true чтобы PM не
           * перевешивал focus на view.dom когда input получил фокус.
           */
          handleClickOn(_view, _pos, node, _nodePos, event) {
            if (node.type.name !== "taskRef") return false;
            const target = event.target as HTMLElement | null;
            if (!target) return false;
            if (target.closest(".task-ref-title-input, .task-ref-status, .task-ref-open")) {
              return true;
            }
            return false;
          },
          handleDOMEvents: {
            /**
             * mousedown на native UI внутри taskRef → возвращаем true.
             * PM в dispatchEvent: `if (!runCustomHandler && handlers[type]) handlers[type](...)`.
             * Если мы вернём true — PM пропускает свой `handlers.mousedown`,
             * НЕ создаёт `new MouseDown(view, pos, event)`, НЕ навешивает
             * на root слушатель mouseup → selectClickedLeaf никогда не
             * запускается. Browser default уже отработал в bubble-стадии
             * (focus на input + caret-position в input.value).
             */
            mousedown(_view, event) {
              const target = event.target as HTMLElement | null;
              if (target?.closest(".task-ref-title-input, .task-ref-status, .task-ref-open")) {
                return true;
              }
              return false;
            },
            // Если focus переходит на native input — это «хорошо», PM
            // не должен пытаться отобрать его обратно на свой view.dom.
            focus(_view, event) {
              const target = event.target as HTMLElement | null;
              if (target?.closest(".task-ref-title-input")) return true;
              return false;
            },
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
