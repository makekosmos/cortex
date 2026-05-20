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
        @focus="onTitleFocus"
        @blur="onTitleBlur"
        @keydown.enter.prevent.stop="commitAndCreateNew"
        @keydown.escape.prevent.stop="cancelAndBlur"
        @keydown.delete.stop="onTitleKeyDelete"
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
    | { state?: { selection?: { from: number; to: number; empty: boolean; constructor: { name: string } } } }
    | undefined;
  const sel = editor?.state?.selection;
  if (!sel || sel.empty) {
    isRangeSelected.value = false;
    return;
  }
  // NodeSelection обрабатывается через `selected` prop — не дублируем класс.
  if (sel.constructor.name === "NodeSelection") {
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
async function commitAndCreateNew(): Promise<void> {
  // Если текущая task пустая — Enter = «выход из task'ов», создаём
  // обычный параграф ниже + фокус.
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

  // Pre-generate UUID + async create task_obj. Async create не блокирует
  // sync insertion node (race покроет NodeView retry в loadTask).
  const newTaskId = crypto.randomUUID();
  void edenApi.createTask(sourceNoteId, "", newTaskId);

  // ВАЖНО: await commit BEFORE re-fetching myPos. Patch task_obj
  // диспатчит tr → positions могут shift'нуться. Берём актуальную
  // позицию ПОСЛЕ commit.
  await commitInputValue();

  const getPos = props.getPos;
  if (typeof getPos !== "function") return;
  const myPos = getPos();
  if (typeof myPos !== "number") {
    console.warn("[eden TaskRef] commitAndCreateNew: stale getPos");
    return;
  }
  const node = props.node as { nodeSize: number };

  const insertPos = myPos + node.nodeSize;
  const newNode = editor.state.schema.nodes.taskRef?.create({
    taskId: newTaskId,
    autoFocus: true,
  });
  if (!newNode) return;
  const tr = editor.view.state.tr.insert(insertPos, newNode);
  editor.view.dispatch(tr);
}

/**
 * Enter на пустой task — создаём parag ниже + фокусируем editor туда.
 * Эмулирует Obsidian: Enter на пустом checkbox-item «выходит» из списка.
 */
function exitToNewParagraph(): void {
  const editor = props.editor as {
    state: { schema: { nodes: { paragraph?: { create: () => unknown } } } };
    view: { dispatch: (tr: unknown) => void; state: { tr: { insert: (pos: number, node: unknown) => unknown } } };
    commands?: { focus?: (pos: number) => void };
  };
  const getPos = props.getPos;
  const node = props.node as { nodeSize: number };
  if (typeof getPos !== "function") return;
  const myPos = getPos();
  if (typeof myPos !== "number") return;
  const para = editor.state.schema.nodes.paragraph?.create();
  if (!para) return;
  const insertPos = myPos + node.nodeSize;
  const tr = editor.view.state.tr.insert(insertPos, para);
  editor.view.dispatch(tr);
  nextTick(() => editor.commands?.focus?.(insertPos + 1));
}

function cancelAndBlur(): void {
  skipNextBlurCommit = true;
  titleInputRef.value?.blur();
}

/**
 * Backspace на пустом title — удалить TaskRef node (и связанный task_obj
 * через soft-delete, чтобы не висеть orphan'ом в Delphi).
 */
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
});

onBeforeUnmount(() => {
  unsubscribe?.();
  selectionUpdateOff?.();
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

/* Range selection highlight — NodeView сам слушает editor.selectionUpdate
   и выставляет `is-range-selected` класс на NodeViewWrapper. Reactive
   refs Vue гарантируют что класс применится корректно (vs Decoration.node,
   который вешает class на DOM мимо Vue'шного render). */
.task-ref-node.is-range-selected,
.task-ref-node.is-node-selected {
  background: var(--selection-bg, rgba(53, 132, 228, 0.25));
  border-radius: 2px;
}

.task-ref-node.is-range-selected .task-ref-row,
.task-ref-node.is-node-selected .task-ref-row {
  background: transparent;
}

/* ProseMirror native NodeSelection — на всякий случай. */
.ProseMirror-selectednode.task-ref-node {
  background: var(--selection-bg, rgba(53, 132, 228, 0.25));
  border-radius: 2px;
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
