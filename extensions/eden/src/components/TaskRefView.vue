<template>
  <NodeViewWrapper
    class="task-ref-node"
    :class="{
      'is-completed': isCompleted,
      'is-cancelled': isCancelled,
      'is-terminated': isTerminated,
      'is-loading': loading,
      'is-missing': missing,
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
        @click="toggleDoneStatus"
      />
      <div
        v-if="!editingTitle"
        class="task-ref-title"
        :class="{ 'task-ref-title-empty': !title }"
        :title="missing ? 'Задача удалена' : 'Клик — переименовать, → — открыть, ПКМ — статус'"
        @click="enterEditMode"
      >
        {{ title || (missing ? "Задача удалена" : "Пустая задача") }}
      </div>
      <input
        v-else
        ref="titleInputRef"
        v-model="editingTitleValue"
        class="task-ref-title-input"
        type="text"
        placeholder="Что нужно сделать?"
        @blur="commitTitle"
        @keydown.enter.prevent="commitTitle"
        @keydown.escape.prevent="cancelEditTitle"
        @keydown.delete="onTitleKeyDelete"
      />
      <button
        v-if="!missing"
        class="task-ref-open"
        type="button"
        title="Открыть задачу"
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef } from "vue";
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
// missing = task_obj был удалён (или never существовал). NodeView показывает
// dimmed placeholder вместо тихого исчезновения — юзер видит что ссылка
// повисла, может удалить node. Также возникает кратковременно когда input
// rule `[ ] ` асинхронно создаёт task_obj — см. retryLoad ниже.
const missing = ref(false);

const editingTitle = ref(false);
const editingTitleValue = ref("");
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

function enterEditMode(): void {
  if (missing.value) return;
  editingTitleValue.value = title.value;
  editingTitle.value = true;
  nextTick(() => {
    titleInputRef.value?.focus();
    titleInputRef.value?.select();
  });
}

/**
 * Backspace на пустом title — удалить TaskRef node (и связанный task_obj
 * через soft-delete, чтобы не висеть orphan'ом в Delphi).
 */
function onTitleKeyDelete(e: KeyboardEvent): void {
  if (e.key !== "Backspace") return;
  if (editingTitleValue.value !== "") return;
  e.preventDefault();
  editingTitle.value = false;
  const id = taskId.value;
  // Soft-delete task_obj в ARK, потом удаляем node. Порядок важен: если
  // удалить node сначала, NodeView unmount'ится и subscribe не успеет
  // обработать ack. Делаем async без await — Eden навигация быстрее.
  if (id) {
    void edenApi.softDeleteTask(id).catch((err) => {
      console.warn("[eden TaskRef] soft delete failed", id, err);
    });
  }
  props.deleteNode();
}

async function commitTitle(): Promise<void> {
  if (!editingTitle.value) return;
  const id = taskId.value;
  const nextTitle = editingTitleValue.value.trim();
  editingTitle.value = false;
  if (!id || nextTitle === title.value) return;
  // Optimistic.
  const prev = title.value;
  title.value = nextTitle;
  try {
    await edenApi.patchTask(id, { title: nextTitle });
  } catch (err) {
    console.warn("[eden TaskRef] title commit failed", id, err);
    title.value = prev;
  }
}

function cancelEditTitle(): void {
  editingTitle.value = false;
}

onMounted(() => {
  void loadTask();
  // Если node только что вставлен (через slash `/задача` или `[ ] ` input
  // rule) — autoFocus=true. Сразу в edit mode, чтобы юзер начал печатать
  // title не делая дополнительный double-click. Сбрасываем atrr через
  // updateAttributes чтобы re-mount (например после autosave reload)
  // не входил снова в editing.
  if (props.node?.attrs?.autoFocus) {
    enterEditMode();
    props.updateAttributes({ autoFocus: false });
  }
  unsubscribe = edenApi.subscribeObjectChanges((payload) => {
    if (payload.id !== taskId.value) return;
    if (payload.event === "object_deleted") {
      missing.value = true;
      return;
    }
    // object_upserted — re-fetch. Не дёргаем если юзер сейчас редактирует
    // title локально (избегаем перезаписи поверх его ввода).
    if (editingTitle.value) return;
    void loadTask();
  });
});

onBeforeUnmount(() => {
  unsubscribe?.();
});
</script>

<style scoped>
.task-ref-node {
  /* Без выраженного outer margin — TipTap paragraph spacing уже создаёт
     ритм, дополнительный margin делает блок «парящим» относительно текста. */
  margin: 0;
}

.task-ref-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  /* Никакого horizontal padding — title должен выравниваться по той же
     левой кромке что и обычный текст параграфа. Vertical 1px для
     baseline alignment с paragraph height. */
  padding: 1px 0;
  border-radius: 4px;
  transition: background-color 120ms ease;
  position: relative;
}

/* Range selection highlight — когда юзер тянет text selection из
   соседнего параграфа и захватывает taskRef, ProseMirror plugin вешает
   класс. Цвет = TipTap дефолтная text selection (через ::selection
   CSS var доступно не везде, поэтому захардкоженный fallback). */
.task-ref-range-selected .task-ref-row {
  background: var(--selection-bg, rgba(53, 132, 228, 0.25));
  border-radius: 2px;
}

/* NodeSelection (юзер кликнул на node сам) — ProseMirror ставит класс. */
.ProseMirror-selectednode.task-ref-node .task-ref-row {
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

.task-ref-title {
  flex: 1 1 auto;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  line-height: 1.4;
  color: var(--foreground, #1a1a1a);
  transition: opacity 120ms ease, color 120ms ease;
}

/* Completed — только затемнение, без зачёркивания. Зачёркивание оставлено
   для is-cancelled (отменённая, не выполненная). */
.is-completed .task-ref-title {
  opacity: 0.5;
  color: var(--muted-foreground, #888);
}

/* Cancelled — зачёркивание + затемнение. Семантически отличается от
   completed: задача НЕ выполнена, она снята с повестки. */
.is-cancelled .task-ref-title {
  text-decoration: line-through;
  opacity: 0.5;
  color: var(--muted-foreground, #888);
}

.task-ref-title-empty {
  color: var(--muted-foreground, #888);
  font-style: italic;
}

.is-missing .task-ref-title {
  color: var(--destructive-foreground, #b14040);
  text-decoration: line-through;
  opacity: 0.7;
}

.task-ref-title-input {
  flex: 1 1 auto;
  font-size: 0.95rem;
  line-height: 1.4;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--foreground, #1a1a1a);
  outline: none;
  font-family: inherit;
}

.task-ref-title-input::placeholder {
  color: var(--muted-foreground, #888);
  opacity: 0.6;
}

.is-loading .task-ref-title {
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
