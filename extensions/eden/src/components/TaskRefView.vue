<template>
  <NodeViewWrapper
    class="task-ref-node"
    :class="{
      'is-completed': isCompleted,
      'is-cancelled': isCancelled,
      'is-terminated': isTerminated,
      'is-loading': loading,
      'is-missing': missing,
      'is-range-selected': isRangeSelected,
      'is-node-selected': selected,
    }"
    :data-task-id="taskId"
  >
    <div
      class="task-ref-row"
      contenteditable="false"
      @mousedown="onRowMouseDown"
      @contextmenu.prevent="openContextMenu"
    >
      <TaskStatusIcon
        class="task-ref-status"
        :status="status"
        :disabled="loading || missing"
        :aria-label="TASK_STATUS_LABELS[status]"
        @mousedown.stop
        @click="toggleDoneStatus"
      />
      <!-- Always-input pattern: ноль mode toggle между display и edit,
           click ставит каретку нативно (как в обычном тексте). Input
           styled под обычный текст — никаких рамок/выделения, без
           cursor: pointer. Read-only когда missing (удалённая задача). -->
      <input
        ref="titleInputRef"
        v-model="titleInputValue"
        class="task-ref-title-input"
        :class="{ 'task-ref-title-input-empty': !titleInputValue }"
        type="text"
        :placeholder="missing ? 'Задача удалена' : 'Пустая задача'"
        :readonly="missing"
        :title="missing ? 'Задача удалена' : 'ПКМ — статус, → — открыть'"
        @mousedown="onTitleMouseDown"
        @focus="onTitleFocus"
        @blur="onTitleBlur"
        @keydown.enter.prevent.stop="onEnterKey"
        @keydown.escape.prevent.stop="cancelAndBlur"
        @keydown.delete.stop="onTitleKeyDelete"
        @keydown.up="onTitleArrowVertical"
        @keydown.down="onTitleArrowVertical"
      />
      <button
        v-if="!missing"
        class="task-ref-open"
        type="button"
        title="Открыть задачу"
        @mousedown.stop
        @click.stop="openTaskPage"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <path
            d="M3 7 L11 7 M7.5 3.5 L11 7 L7.5 10.5"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
    </div>
    <ContextMenu
      :open="contextMenu.isOpen.value"
      :x="contextMenu.x.value"
      :y="contextMenu.y.value"
      @close="contextMenu.close"
    >
      <ContextMenuItem
        v-for="opt in TASK_STATUSES"
        :key="opt"
        :class="{ 'task-ref-menu-item-active': opt === status }"
        @click="setStatus(opt); contextMenu.close()"
      >
        <span class="task-ref-menu-icon">
          <TaskStatusIcon :status="opt" :aria-label="''" tabindex="-1" />
        </span>
        {{ TASK_STATUS_LABELS[opt] }}
      </ContextMenuItem>
    </ContextMenu>
  </NodeViewWrapper>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import { Selection } from "@tiptap/pm/state";
import { ContextMenu, ContextMenuItem, useContextMenu } from "@kosmos/visuals";
import { edenApi } from "@/lib/edenApi";
import TaskStatusIcon from "@/components/TaskStatusIcon.vue";
import {
  TASK_STATUSES,
  TASK_STATUS_LABELS,
  TASK_STATUS_DEFAULT,
  normalizeStatus,
  getStatusCategory,
  type TaskStatus,
} from "@/lib/taskStatus";

// Internal placeholder для пустой задачи (хранится в ARK как title, чтобы
// Delphi не показывал empty-row). В Eden input трактует его как «пусто»
// для UX-целей: input value = "" + placeholder вместо реального текста.
const EMPTY_PLACEHOLDER_TITLE = "Пустая задача";

const props = defineProps(nodeViewProps);

const taskId = computed<string | null>(() => {
  const v = props.node?.attrs?.taskId;
  return typeof v === "string" && v.length > 0 ? v : null;
});

const title = ref("");
const status = ref<TaskStatus>(TASK_STATUS_DEFAULT);
const isCompleted = computed(() => status.value === "done");
const isCancelled = computed(() => status.value === "canceled");
const isTerminated = computed(() => getStatusCategory(status.value) !== "open");
const loading = ref(true);
const contextMenu = useContextMenu<null>();

// Always-input pattern (см. template). titleInputValue — отображаемая в
// input строка. Если ARK хранит placeholder ("Пустая задача") — показываем
// пустоту (input.value = ""), placeholder атрибут подставит текст.
const titleInputValue = ref("");
const isInputFocused = ref(false);
// Flag для отличия «commit on blur» vs «cancel on Escape» — Escape тоже
// генерит blur, но мы не хотим в этом случае писать в ARK.
let skipNextBlurCommit = false;

function displayValueFromTitle(t: string): string {
  return t === EMPTY_PLACEHOLDER_TITLE ? "" : t;
}

// Selection-aware highlight: каждый NodeView подписан на editor's
// selectionUpdate, считает свою позицию в текущей selection и помечает
// isRangeSelected. Это альтернатива `Decoration.node` который теряется
// в Vue NodeView'ах (Vue рулит wrapper'ом, декорация-class не применяется).
const isRangeSelected = ref(false);

function recomputeRangeSelection(): void {
  // props.editor может быть undefined в SSR / edge cases.
  const editor = props.editor as
    | { state?: { selection?: any } }
    | undefined;
  const sel = editor?.state?.selection;
  if (!sel || sel.empty) {
    isRangeSelected.value = false;
    return;
  }
  // NodeSelection: PM ставит `from=pos, to=pos+nodeSize`, и `selected`
  // prop уже отвечает за подсветку самой выбранной ноды. Если делать
  // intersection-check для соседних блоков — граница `to=from+nodeSize`
  // touch'ает соседний atom block'а и подсвечивает его как range-selected.
  // Раньше детектили через `sel.constructor.name === "NodeSelection"`,
  // но это ломается в production-build после минификации (constructor
  // names перебиты). Надёжно: проверяем наличие .node property — оно
  // есть у NodeSelection и AllSelection (всегда не TextSelection), и
  // оба эти типа должны исключаться из range-overlap-highlight.
  if ((sel as any).node !== undefined) {
    isRangeSelected.value = false;
    return;
  }
  const nodePos = typeof props.getPos === "function" ? props.getPos() : null;
  if (typeof nodePos !== "number") {
    isRangeSelected.value = false;
    return;
  }
  const nodeSize = (props.node as { nodeSize?: number })?.nodeSize ?? 1;
  const nodeFrom = nodePos;
  const nodeTo = nodePos + nodeSize;
  // Intersection: либо selection полностью покрывает node, либо
  // partial overlap. Atom block считается «включён» если хотя бы один
  // его край внутри selection.
  isRangeSelected.value = nodeFrom < sel.to && nodeTo > sel.from;
}
// missing = task_obj был удалён (или never существовал). NodeView показывает
// dimmed placeholder вместо тихого исчезновения — юзер видит что ссылка
// повисла, может удалить node. Также возникает кратковременно когда input
// rule `[ ] ` асинхронно создаёт task_obj — см. retryLoad ниже.
const missing = ref(false);

const titleInputRef = useTemplateRef<HTMLInputElement>("titleInputRef");

let unsubscribe: (() => void) | null = null;

async function loadTask(opts: { allowRetry?: boolean } = {}): Promise<void> {
  const id = taskId.value;
  if (!id) {
    loading.value = false;
    missing.value = true;
    return;
  }
  try {
    const obj = await edenApi.getTask(id);
    if (!obj) {
      // Race: input rule `[ ] ` синхронно вставляет TaskRef, потом async
      // createTask пишет task_obj. NodeView mount'ится первым — get_object
      // возвращает null. Retry один раз через 350ms покрывает типичный
      // round-trip (`~30-100ms`).
      if (opts.allowRetry !== false) {
        window.setTimeout(() => void loadTask({ allowRetry: false }), 350);
        return;
      }
      missing.value = true;
      title.value = "";
      status.value = TASK_STATUS_DEFAULT;
      return;
    }
    missing.value = false;
    title.value = obj.title ?? "";
    // Sync input value с server'ным title, ЕСЛИ юзер сейчас не печатает.
    // Иначе clobber'ом стер бы его пендинг ввод.
    if (!isInputFocused.value) {
      titleInputValue.value = displayValueFromTitle(title.value);
    }
    const propsRaw = (obj.propsJson ?? {}) as Record<string, unknown>;
    status.value = normalizeStatus({
      status: propsRaw.status,
      is_completed: propsRaw.is_completed,
      is_cancelled: propsRaw.is_cancelled,
    });
  } catch (err) {
    console.warn("[eden TaskRef] load failed", id, err);
    missing.value = true;
  } finally {
    loading.value = false;
  }
}

/**
 * Click на иконку статуса — простой toggle done ↔ todo (как в Delphi).
 * Для остальных 5 статусов юзер использует ПКМ-меню.
 */
async function toggleDoneStatus(): Promise<void> {
  if (missing.value) return;
  const next: TaskStatus = isCompleted.value ? "todo" : "done";
  await setStatus(next);
}

async function setStatus(next: TaskStatus): Promise<void> {
  const id = taskId.value;
  if (!id || missing.value) return;
  const prev = status.value;
  // Optimistic: обновляем local state СРАЗУ, иначе UI лагает на click.
  status.value = next;
  try {
    await edenApi.patchTask(id, { status: next });
  } catch (err) {
    console.warn("[eden TaskRef] setStatus failed", id, err);
    status.value = prev;
    void loadTask();
  }
}

function openContextMenu(e: MouseEvent): void {
  if (missing.value) return;
  contextMenu.open(e, null);
}

function openTaskPage(): void {
  const id = taskId.value;
  if (!id || missing.value) return;
  // TODO(Pattern B Phase 2): eden.navigateTo(id) когда task page route готов.
  // Пока — открываем через event на window для App.vue/store.
  window.dispatchEvent(
    new CustomEvent("eden:open-task", { detail: { taskId: id } }),
  );
}

/**
 * Защита от PM-кражи focus'а: PM на mouseup внутри view.dom делает
 * `view.focus()` через свой selection-sync, и каретка уезжает с input
 * на саму .ProseMirror. Перехватываем mousedown на input ДО PM (через
 * @mousedown.stop в template), мемоизируем где должна стоять каретка
 * (по click coords через document.caretRangeFromPoint), и через rAF
 * (после PM transactions) форсим focus + selectionStart обратно.
 */
// Защитник от PM-кражи focus'а. PM на mouseup внутри view.dom через
// MouseDown class → selectClickedLeaf / Selection.near → updateSelection →
// view.focus() — focus уезжает с нашего нативного `<input>` на
// .ProseMirror DIV, каретка теряется. handleClickOn/handleDOMEvents в
// плагине не помогают полностью (PM использует selectionchange listener
// + delayed setTimeout selectionToDOM, обходя plugin-handlers).
//
// Стратегия: blur-listener на input. Если focus ушёл именно на
// .ProseMirror — это PM-кража, возвращаем focus + сохранённый caret
// через rAF. Caret снимаем в onTitleMouseDown (тогда нативный focus
// уже расставил его по координате клика).
// Focus-defender: PM-mouseup → selectClickedLeaf / Selection.near →
// updateSelection → view.focus() уводит focus с нативного input на
// .ProseMirror DIV. Возвращаем его обратно, НО только если пользователь
// сам не запросил уход.
//
// Активируем defender на mousedown по input'у. Деактивируем как только
// document получит mousedown с target ВНЕ нашего input (юзер кликнул
// куда-то ещё намеренно). Document capture-listener срабатывает раньше
// чем blur события input'а — успеем сбросить флаг до того как defender
// решит refocus'нуть.
let focusDefenderActive = false;
let blurDefenderOff: (() => void) | null = null;
let mousedownDocumentOff: (() => void) | null = null;
let lastCaretBeforeBlur: number | null = null;

function onTitleMouseDown(_e: MouseEvent): void {
  const input = titleInputRef.value;
  if (!input || missing.value) return;
  input.focus();
  focusDefenderActive = true;
}

/**
 * mousedown по самой task-ref-row (padding'и между input'ом и кнопками,
 * пустая правая часть после короткого title'а): пользователь явно хотел
 * взаимодействовать с задачей. Фокусим input, чтобы клик не «уходил
 * впустую» в .ProseMirror.
 */
function onRowMouseDown(e: MouseEvent): void {
  const target = e.target as HTMLElement | null;
  // На сам input или интерактивные кнопки — не вмешиваемся, их собственные
  // handler'ы отработают.
  if (target?.closest(".task-ref-title-input, .task-ref-status, .task-ref-open")) return;
  const input = titleInputRef.value;
  if (!input || missing.value) return;
  input.focus();
  // Кликнули по margin'у справа — каретку в конец текста.
  const len = input.value.length;
  input.setSelectionRange(len, len);
  focusDefenderActive = true;
}

function defendFocus(): void {
  const input = titleInputRef.value;
  if (!input) return;

  // Document mousedown в CAPTURE-фазе: если target — не наш input, юзер
  // намеренно кликнул в другое место. Сбрасываем defender, focus уйдёт
  // нормально.
  const handleDocMouseDown = (event: MouseEvent) => {
    const target = event.target as HTMLElement | null;
    // Деактивируем defender только если клик ушёл ВНЕ нашего task-ref-row.
    // Клик внутри row (input, paddings, buttons) — наше UI, держим input
    // focused. Если ушёл куда-то ещё (paragraph, другой task) — отпускаем
    // focus, юзер сам выбрал куда переключиться.
    const row = input.closest(".task-ref-row");
    if (!row || !target || !row.contains(target)) {
      focusDefenderActive = false;
    }
  };
  document.addEventListener("mousedown", handleDocMouseDown, { capture: true });
  mousedownDocumentOff = () =>
    document.removeEventListener("mousedown", handleDocMouseDown, { capture: true } as EventListenerOptions);

  const handleBlur = (event: FocusEvent) => {
    if (!focusDefenderActive) return;
    lastCaretBeforeBlur = input.selectionStart;
    const next = event.relatedTarget as HTMLElement | null;
    if (next && next.classList?.contains("ProseMirror")) {
      requestAnimationFrame(() => {
        if (!focusDefenderActive) return;
        input.focus();
        if (lastCaretBeforeBlur !== null) {
          input.setSelectionRange(lastCaretBeforeBlur, lastCaretBeforeBlur);
        }
      });
    }
  };
  input.addEventListener("blur", handleBlur);
  blurDefenderOff = () => input.removeEventListener("blur", handleBlur);
}

function onTitleFocus(): void {
  isInputFocused.value = true;
}

async function onTitleBlur(): Promise<void> {
  isInputFocused.value = false;
  if (skipNextBlurCommit) {
    skipNextBlurCommit = false;
    // Revert local value к server'ному title display.
    titleInputValue.value = displayValueFromTitle(title.value);
    return;
  }
  await commitInputValue();
}

async function commitInputValue(): Promise<void> {
  const id = taskId.value;
  if (!id || missing.value) return;
  const next = titleInputValue.value.trim();
  const prevTitleDisplayed = displayValueFromTitle(title.value);
  if (next === prevTitleDisplayed) return;
  // Optimistic UI: меняем local title до ack от сервера.
  // patchTask нормализует empty → "Пустая задача" (см. shim).
  title.value = next || EMPTY_PLACEHOLDER_TITLE;
  try {
    await edenApi.patchTask(id, { title: next });
  } catch (err) {
    console.warn("[eden TaskRef] title commit failed", id, err);
    // Не пытаемся откатить — loadTask на entity_changed выровняет.
  }
}

function commitAndBlur(): void {
  titleInputRef.value?.blur();
}

/**
 * Enter в title input — commit + создать НОВУЮ task ниже + фокус на неё
 * (Obsidian-pattern для checked list items). Если current task пустая —
 * выходим из task mode вместо создания (стандартный escape поведение).
 */
// Re-entry guard: typematic Enter / IME / queued events могут вызвать
// commitAndCreateNew дважды пока первый ещё await'ит patchTask, и каждый
// делает insert → 2 пустых taskRef'а вместо одной. Flag блокирует.
let creatingNew = false;

/**
 * Wrapper над commitAndCreateNew который ловит typematic Enter и
 * IME composition. Browser выставляет `event.repeat = true` для повторов
 * (после первого ~30ms). Если key.repeat — не делаем ничего. Это
 * фиксит баг «два пустых блока создаётся когда пользователь долго
 * жмёт Enter» — typematic Enter на autoFocus'нутой новой task'е
 * (с пустым title) триггерил `exitToNewParagraph` → пустой `<p>` ниже.
 */
function onEnterKey(e: KeyboardEvent): void {
  if (e.repeat) return;
  if (e.isComposing) return; // IME composition — Enter подтверждает символ
  commitAndCreateNew();
}

function commitAndCreateNew(): void {
  if (creatingNew) return;
  creatingNew = true;
  try {
    // Пустая task → Enter = «выход в обычный параграф» (Obsidian pattern).
    if (titleInputValue.value.trim() === "") {
      exitToNewParagraph();
      return;
    }

    const editor = props.editor as {
      state: { schema: { nodes: { taskRef?: { create: (attrs: Record<string, unknown>) => unknown } } } };
      view: { dispatch: (tr: unknown) => void; state: { tr: { insert: (pos: number, node: unknown) => unknown } } };
    };
    const extension = props.extension as { options?: { getSourceNoteId?: () => string | null } } | undefined;
    const sourceNoteId = extension?.options?.getSourceNoteId?.() ?? null;
    if (!sourceNoteId) {
      console.warn("[eden TaskRef] commitAndCreateNew: no sourceNoteId");
      return;
    }

    // SYNC flow: getPos сейчас, insert сейчас, async writes — потом.
    // Раньше был await commitInputValue() до getPos — это создавало
    // 50-150ms окно re-entry, второй Enter → второй insert.
    const getPos = props.getPos;
    if (typeof getPos !== "function") return;
    const myPos = getPos();
    if (typeof myPos !== "number") {
      console.warn("[eden TaskRef] commitAndCreateNew: stale getPos");
      return;
    }
    // Находим реальный taskRef в документе по taskId — не доверяем
    // props.node.nodeSize (PM иногда возвращал 2 вместо 1 для atom node,
    // и `myPos + 2` падал ВНУТРЬ следующего параграфа → tr.insert split'ил
    // параграф → 4 блока вместо 3).
    const doc = editor.view.state.doc as any;
    const targetTaskId = (props.node as any).attrs?.taskId;
    let insertPos = -1;
    doc.descendants((child: any, pos: number) => {
      if (insertPos !== -1) return false;
      if (child.type?.name === "taskRef" && child.attrs?.taskId === targetTaskId) {
        insertPos = pos + child.nodeSize;
        return false;
      }
      return true;
    });
    if (insertPos === -1) {
      console.warn("[eden TaskRef] commitAndCreateNew: не нашли taskRef в документе");
      return;
    }

    const newTaskId = crypto.randomUUID();
    const newNode = editor.state.schema.nodes.taskRef?.create({
      taskId: newTaskId,
      autoFocus: true,
    });
    if (!newNode) return;

    // В OLD-документе сразу после нашего taskRef'а: если там пустой
    // параграф и он же последний блок doc'а — удалим его. Это убирает
    // «висящую пустую строку» под новой task'ой (slash-команда оставляет
    // trailing paragraph, каждый Enter создаёт task ПЕРЕД ним).
    // Если за нашим taskRef'ом висит trailing empty paragraph (например
    // оставшийся от slash-команды), сразу заменим его новой task'ой —
    // тогда под текущей фокусной task'ой не висит пустая строка.
    let deleteTail: { from: number; to: number } | null = null;
    const tailInOld = doc.nodeAt(insertPos);
    if (
      tailInOld &&
      tailInOld.type?.name === "paragraph" &&
      tailInOld.content?.size === 0 &&
      insertPos + tailInOld.nodeSize === doc.content.size
    ) {
      deleteTail = { from: insertPos, to: insertPos + tailInOld.nodeSize };
    }

    // Если за нашим taskRef'ом висит trailing empty paragraph (slash-команда
    // оставила), используем replaceWith вместо insert — это атомарно
    // заменит «всё от insertPos до конца» новой task'ой. Иначе обычный
    // insert, paragraph PM добавит обратно автоматически (gapcursor / схема).
    const tr = editor.view.state.tr;
    if (deleteTail) {
      tr.replaceWith(deleteTail.from, deleteTail.to, newNode);
    } else {
      tr.insert(insertPos, newNode);
    }
    editor.view.dispatch(tr);

    // 2. Optimistic local commit + background async ARK writes —
    //    fire-and-forget. NodeView новой task'и retry'ит loadTask
    //    пока createTask не завершится.
    // Захватываем OLD title до set, чтобы потом сравнить (иначе мы
    // зануляем diff и patch никогда не отрабатывает).
    const localTitle = titleInputValue.value.trim();
    const oldDisplayTitle = displayValueFromTitle(title.value);
    title.value = localTitle || EMPTY_PLACEHOLDER_TITLE;
    const currentTaskId = taskId.value;
    void edenApi.createTask(sourceNoteId, "", newTaskId).catch((err) => {
      console.warn("[eden TaskRef] createTask failed", err);
    });
    if (currentTaskId && localTitle !== oldDisplayTitle) {
      void edenApi.patchTask(currentTaskId, { title: localTitle }).catch((err) => {
        console.warn("[eden TaskRef] patchTask failed", err);
      });
    }
  } finally {
    // Snapshot creatingNew clear в next tick, чтобы успеть пройти
    // potential re-entry events на этом же микро-такте.
    nextTick(() => {
      creatingNew = false;
    });
  }
}

/**
 * Enter на пустой task — заменяем сам taskRef на пустой paragraph
 * («Enter чистит строку»). Заодно soft-delete task_obj в ARK, чтобы
 * пустая задача не висела orphan'ом в Delphi.
 */
function exitToNewParagraph(): void {
  const editor = props.editor as any;
  const getPos = props.getPos;
  if (typeof getPos !== "function") return;
  const para = editor.state.schema.nodes.paragraph?.create();
  if (!para) return;
  const doc = editor.view.state.doc;
  const targetTaskId = (props.node as any).attrs?.taskId;
  let from = -1;
  let to = -1;
  doc.descendants((child: any, pos: number) => {
    if (from !== -1) return false;
    if (child.type?.name === "taskRef" && child.attrs?.taskId === targetTaskId) {
      from = pos;
      to = pos + child.nodeSize;
      return false;
    }
    return true;
  });
  if (from === -1) return;
  const tr = editor.view.state.tr.replaceWith(from, to, para);
  editor.view.dispatch(tr);
  nextTick(() => editor.commands?.focus?.(from + 1));

  // Soft-delete task_obj в ARK — пустая задача не должна торчать в Delphi.
  if (targetTaskId) {
    void edenApi.softDeleteTask(targetTaskId).catch((err) => {
      console.warn("[eden TaskRef] softDeleteTask (Enter on empty) failed", err);
    });
  }
}

function cancelAndBlur(): void {
  skipNextBlurCommit = true;
  titleInputRef.value?.blur();
}

/**
 * Backspace на пустом title — удалить TaskRef node (и связанный task_obj
 * через soft-delete, чтобы не висеть orphan'ом в Delphi).
 */
/**
 * ArrowUp/ArrowDown на title input — стандартная PM-навигация между
 * блоками. Native `<input>` сам по себе глотает arrow keys (single-line
 * input двигает caret внутри value, что бесполезно). Перехватываем,
 * находим соседний textblock через `Selection.findFrom` относительно
 * границы текущего TaskRef'а и переводим туда PM TextSelection. Если
 * соседний блок — тоже TaskRef, его NodeView сам сфокусит свой input
 * через autoFocus-attr-flow… нет, NodeView не реагирует на selection
 * snap, так что DOM-фокусим input напрямую.
 *
 * Selection.findFrom отвечает за корректный skip atom/non-textblock
 * нод и возврат валидной TextSelection. Если на пути одни atom-блоки —
 * findFrom вернёт null → ничего не делаем (нет места для текстовой
 * каретки в этом направлении).
 */
/**
 * DOM-фокус на title input соседнего TaskRef'а по taskId. Используется когда PM
 * findFrom скипает atom-блок или мы напрямую знаем siblingTaskId. Возвращает
 * true если нашли input и сфокусили.
 *
 * Trap #4: focus-defender вернёт focus обратно на текущий input если не сбросить
 * флаг ДО `.focus()`. Порядок критичен.
 */
function focusTaskRefInputDOM(view: { dom: HTMLElement }, taskRefId: string): boolean {
  const input = view.dom.querySelector(
    `[data-task-id="${taskRefId}"] .task-ref-title-input`,
  ) as HTMLInputElement | null;
  if (!input) return false;
  focusDefenderActive = false;
  input.focus();
  const len = input.value.length;
  input.setSelectionRange(len, len);
  return true;
}

/**
 * Резолвит позицию TaskRef'а в актуальном doc по taskId (Trap #1: props.node.nodeSize
 * врёт). Сначала top-level loop — тогда возвращаем topIndex для adjacency-check'а.
 * Если не нашли на верхнем уровне (вложен в blockquote и т.п.) — fallback на
 * descendants с topIndex = -1.
 */
function findTaskRefInDoc(
  doc: any,
  taskRefId: string,
): { from: number; to: number; topIndex: number } | null {
  let runningPos = 0;
  for (let i = 0; i < doc.childCount; i++) {
    const child = doc.child(i);
    if (child.type?.name === "taskRef" && child.attrs?.taskId === taskRefId) {
      return { from: runningPos, to: runningPos + child.nodeSize, topIndex: i };
    }
    runningPos += child.nodeSize;
  }
  let from = -1;
  let to = -1;
  doc.descendants((child: any, pos: number) => {
    if (from !== -1) return false;
    if (child.type?.name === "taskRef" && child.attrs?.taskId === taskRefId) {
      from = pos;
      to = pos + child.nodeSize;
      return false;
    }
    return true;
  });
  if (from === -1) return null;
  return { from, to, topIndex: -1 };
}

function onTitleArrowVertical(e: KeyboardEvent): void {
  if (e.isComposing) return;
  const editor = props.editor as
    | { view?: { state: any; dispatch: (tr: any) => void; dom: HTMLElement }; commands?: { focus?: (pos?: number) => void } }
    | undefined;
  if (!editor?.view) return;
  const getPos = props.getPos;
  if (typeof getPos !== "function") return;

  const doc = editor.view.state.doc;
  const targetTaskId = (props.node as any).attrs?.taskId;
  const located = findTaskRefInDoc(doc, targetTaskId);
  if (!located) return;
  const { from: myFrom, to: myTo, topIndex: myTopIndex } = located;

  const up = e.key === "ArrowUp";

  // Прямой adjacency: если соседний top-level node — тоже TaskRef, фокусим
  // его input через DOM (textOnly findFrom его бы скипнул как atom).
  if (myTopIndex !== -1) {
    const siblingIndex = up ? myTopIndex - 1 : myTopIndex + 1;
    if (siblingIndex >= 0 && siblingIndex < doc.childCount) {
      const sibling = doc.child(siblingIndex);
      if (sibling.type?.name === "taskRef") {
        const siblingTaskId = sibling.attrs?.taskId;
        if (siblingTaskId) {
          e.preventDefault();
          e.stopPropagation();
          if (focusTaskRefInputDOM(editor.view, siblingTaskId)) return;
        }
      }
    }
  }

  // Иначе — стандартный PM findFrom. textOnly=false чтобы захватить и atom
  // TaskRef (на случай TaskRef через несколько paragraph'ов / в blockquote'е).
  const probePos = up ? myFrom : myTo;
  const $probe = doc.resolve(probePos);
  const next = Selection.findFrom($probe, up ? -1 : 1, false);
  // Edge: findFrom вернул null — нет места для selection в этом направлении.
  // НЕ preventDefault — даём native input стрелкам шанс (IME / single-line
  // caret motion внутри value).
  if (!next) return;

  // Trap #3: detect NodeSelection без constructor.name (минификация перебивает
  // имена в prod-build). Используем наличие .node property — есть у NodeSelection
  // и AllSelection, нет у TextSelection.
  const selNode = (next as any).node;
  if (selNode && selNode.type?.name === "taskRef") {
    const nextTaskId = selNode.attrs?.taskId;
    if (nextTaskId) {
      e.preventDefault();
      e.stopPropagation();
      if (focusTaskRefInputDOM(editor.view, nextTaskId)) return;
    }
  }

  // TextSelection в обычном textblock'е (paragraph / heading / ...).
  e.preventDefault();
  e.stopPropagation();
  focusDefenderActive = false;
  const tr = editor.view.state.tr.setSelection(next).scrollIntoView();
  editor.view.dispatch(tr);
  editor.view.focus();
}

function onTitleKeyDelete(e: KeyboardEvent): void {
  if (e.key !== "Backspace") return;
  if (titleInputValue.value !== "") return;
  e.preventDefault();
  const id = taskId.value;
  if (id) {
    void edenApi.softDeleteTask(id).catch((err) => {
      console.warn("[eden TaskRef] soft delete failed", id, err);
    });
  }
  const nodePos = typeof props.getPos === "function" ? props.getPos() : null;
  props.deleteNode();
  nextTick(() => {
    const editor = props.editor as { commands?: { focus?: (pos?: number) => void } } | undefined;
    if (typeof nodePos === "number") {
      editor?.commands?.focus?.(nodePos);
    } else {
      editor?.commands?.focus?.();
    }
  });
}

let selectionUpdateOff: (() => void) | null = null;

onMounted(() => {
  void loadTask();
  // Если node только что вставлен (через slash `/задача` или `[ ] ` input
  // rule) — autoFocus=true. Фокусируем input, ставим каретку в конец.
  // Сбрасываем attr через updateAttributes чтобы re-mount (например после
  // autosave reload) не фокусил снова.
  if (props.node?.attrs?.autoFocus) {
    nextTick(() => {
      titleInputRef.value?.focus();
      const len = titleInputValue.value.length;
      titleInputRef.value?.setSelectionRange(len, len);
    });
    props.updateAttributes({ autoFocus: false });
  }
  unsubscribe = edenApi.subscribeObjectChanges((payload) => {
    if (payload.id !== taskId.value) return;
    if (payload.event === "object_deleted") {
      missing.value = true;
      return;
    }
    // object_upserted — re-fetch. Skip если юзер сейчас печатает локально
    // (loadTask внутри проверит isInputFocused и не клобберит ввод).
    void loadTask();
  });

  // Подписка на editor.selectionUpdate для range-highlight. Каждое
  // изменение selection re-checks включён ли этот node в range.
  const editor = props.editor as
    | { on?: (event: string, cb: () => void) => void; off?: (event: string, cb: () => void) => void }
    | undefined;
  if (editor?.on && editor.off) {
    editor.on("selectionUpdate", recomputeRangeSelection);
    recomputeRangeSelection();
    selectionUpdateOff = () => editor.off?.("selectionUpdate", recomputeRangeSelection);
  }

  defendFocus();
});

onBeforeUnmount(() => {
  unsubscribe?.();
  selectionUpdateOff?.();
  blurDefenderOff?.();
  mousedownDocumentOff?.();
});
</script>

<style scoped>
.task-ref-node {
  /* НЕ задаём margin: 0 — это убивает `.ProseMirror > * + * { margin-top: 0.85em }`
     правило (см. Editor.css), из-за чего две задачи подряд слипались без
     gap'а между ними, а между параграфом и задачей gap присутствовал.
     Оставляем naturalный flow — TipTap паттерн margin-top: 0.85em
     одинаково применится к TaskRef и к <p>. */
}

.task-ref-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0;
  border-radius: 4px;
  transition: background-color 120ms ease;
  position: relative;
  /* Inherit от .ProseMirror — то же font-size (16px) и line-height (1.65)
     что у параграфов. Это даёт visual size + vertical rhythm идентичный
     <p>. Раньше у row было fixed font-size 0.95rem + line-height 1.4 →
     ~21px высота vs ~26px у <p>. Юзер видел разницу. */
  font-size: inherit;
  line-height: inherit;
  min-height: 1.65em;
}

/* Highlight'ы при PM-NodeSelection / range-overlap НЕ показываем:
   обычный клик на task случайно ставит NodeSelection (atom block —
   PM выбирает его целиком), и юзер видит «выделение» хотя он
   ничего не выделял. Семантика NodeSelection (Delete удалит блок)
   сохраняется через `selected` prop, просто без визуала. Rubber-band
   selection даёт свой visual через `.kepler-block-selected` (отдельный
   слой — PM Decoration plugin). */
.ProseMirror-selectednode.task-ref-node {
  outline: none;
}

/* TaskStatusIcon подсовывает brand orange (Eden — оранжевый, тот же что
   у bullet/ordered list markers). Кросс-extension `--task-status-accent`
   override на :deep селекторе. */
.task-ref-row :deep(.task-status-icon) {
  --task-status-accent: var(--eden-accent-color);
}

/* Done — затемняем и сам квадратик (не только title), как просил юзер.
   Не используем .task-status-icon--dim (дублирующий механизм) — здесь
   контекстная дополнительная отметка на уровне node-row'а. */
.is-completed .task-ref-status {
  opacity: 0.55;
}

/* ContextMenu item с активным статусом — небольшое отличие чтобы юзер
   видел текущий выбор. */
.task-ref-menu-item-active {
  background: var(--surface-hover, rgba(0, 0, 0, 0.04));
  font-weight: 500;
}

.task-ref-menu-icon {
  display: inline-flex;
  align-items: center;
  margin-right: 8px;
  /* Eden orange внутри context menu тоже. */
  --task-status-accent: var(--eden-accent-color);
}

/* Always-input title — стилизован как обычный текст. Без border, без bg,
   cursor: text (по умолчанию у input — самое то). Никакого pointer/select. */
.task-ref-title-input {
  flex: 1 1 auto;
  /* Inherit от .ProseMirror — size и rhythm как у parameter'а. */
  font-size: inherit;
  line-height: inherit;
  padding: 0;
  margin: 0;
  border: none;
  background: transparent;
  color: var(--foreground, #1a1a1a);
  outline: none;
  font-family: inherit;
  font-weight: inherit;
  /* cursor: text у input нативно — не override'им */
  transition: opacity 120ms ease, color 120ms ease;
  min-width: 0;
}

.task-ref-title-input::placeholder {
  color: var(--muted-foreground, #888);
  opacity: 0.6;
  font-style: italic;
}

/* Completed — затемнение без зачёркивания. */
.is-completed .task-ref-title-input {
  opacity: 0.5;
  color: var(--muted-foreground, #888);
}

/* Cancelled — зачёркивание + затемнение. */
.is-cancelled .task-ref-title-input {
  text-decoration: line-through;
  opacity: 0.5;
  color: var(--muted-foreground, #888);
}

.is-missing .task-ref-title-input {
  color: var(--destructive-foreground, #b14040);
  text-decoration: line-through;
  opacity: 0.7;
  cursor: default;
}

.is-loading .task-ref-title-input {
  color: var(--muted-foreground, #888);
}

/* Arrow button «открыть задачу» — появляется только на hover row'а.
   Inline-flex, чтобы не ломать baseline текста. */
.task-ref-open {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  margin-left: auto;
  padding: 0;
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  color: var(--muted-foreground, #888);
  opacity: 0;
  transition: opacity 120ms ease, color 120ms ease, background-color 120ms ease;
  flex-shrink: 0;
}

.task-ref-row:hover .task-ref-open,
.task-ref-open:focus-visible {
  opacity: 1;
}

.task-ref-open:hover {
  color: var(--eden-accent-color);
  background: var(--surface-hover, rgba(0, 0, 0, 0.04));
}
</style>
