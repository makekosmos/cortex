<template>
  <NodeViewWrapper
    class="task-ref-node"
    :class="{ 'is-completed': isCompleted, 'is-loading': loading, 'is-missing': missing }"
    :data-task-id="taskId"
  >
    <div class="task-ref-row" contenteditable="false">
      <Checkbox
        class="task-ref-checkbox"
        :model-value="isCompleted"
        :disabled="loading || missing"
        :aria-label="isCompleted ? 'Снять отметку' : 'Отметить выполненной'"
        @click.stop
        @update:model-value="toggleCompleted"
      />
      <div
        v-if="!editingTitle"
        class="task-ref-title"
        :class="{ 'task-ref-title-empty': !title }"
        :title="missing ? 'Задача удалена' : 'Клик — открыть, двойной клик — переименовать'"
        @click="handleTitleClick"
        @dblclick="enterEditMode"
      >
        {{ title || (missing ? "Задача удалена" : "Без названия") }}
      </div>
      <input
        v-else
        ref="titleInputRef"
        v-model="editingTitleValue"
        class="task-ref-title-input"
        type="text"
        @blur="commitTitle"
        @keydown.enter.prevent="commitTitle"
        @keydown.escape.prevent="cancelEditTitle"
      />
    </div>
  </NodeViewWrapper>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import { Checkbox } from "@kosmos/visuals";
import { edenApi } from "@/lib/edenApi";

const props = defineProps(nodeViewProps);

const taskId = computed<string | null>(() => {
  const v = props.node?.attrs?.taskId;
  return typeof v === "string" && v.length > 0 ? v : null;
});

const title = ref("");
const isCompleted = ref(false);
const loading = ref(true);
// missing = task_obj был удалён (или never существовал). NodeView показывает
// strikethrough placeholder вместо тихого исчезновения — юзер видит что
// ссылка повисла, может удалить node.
const missing = ref(false);

const editingTitle = ref(false);
const editingTitleValue = ref("");
const titleInputRef = useTemplateRef<HTMLInputElement>("titleInputRef");

// Click-with-double-click-detection: одиночный клик ждёт 250ms перед навигацией,
// чтобы успеть отменить если придёт второй клик. Стандартный паттерн для UI
// где single и double имеют разный intent.
let singleClickTimer: number | null = null;
const SINGLE_CLICK_DELAY_MS = 250;

let unsubscribe: (() => void) | null = null;

async function loadTask(): Promise<void> {
  const id = taskId.value;
  if (!id) {
    loading.value = false;
    missing.value = true;
    return;
  }
  try {
    const obj = await edenApi.getTask(id);
    if (!obj) {
      missing.value = true;
      title.value = "";
      isCompleted.value = false;
      return;
    }
    missing.value = false;
    title.value = obj.title ?? "";
    const propsRaw = (obj.propsJson ?? {}) as Record<string, unknown>;
    isCompleted.value = Boolean(propsRaw.is_completed);
  } catch (err) {
    console.warn("[eden TaskRef] load failed", id, err);
    missing.value = true;
  } finally {
    loading.value = false;
  }
}

async function toggleCompleted(): Promise<void> {
  const id = taskId.value;
  if (!id || missing.value) return;
  const next = !isCompleted.value;
  // Optimistic: обновляем local state СРАЗУ, иначе клик выглядит лагающим.
  // Если patchTask упадёт — re-fetch в catch'е выровняет.
  isCompleted.value = next;
  try {
    await edenApi.patchTask(id, { isCompleted: next });
  } catch (err) {
    console.warn("[eden TaskRef] toggle failed", id, err);
    void loadTask();
  }
}

function handleTitleClick(): void {
  // Если уже есть pending single — пришёл двойной, не дёргаем.
  // (Браузер сам сгенерит dblclick событие.)
  if (singleClickTimer !== null) {
    window.clearTimeout(singleClickTimer);
    singleClickTimer = null;
    return;
  }
  singleClickTimer = window.setTimeout(() => {
    singleClickTimer = null;
    openTaskPage();
  }, SINGLE_CLICK_DELAY_MS);
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
  if (singleClickTimer !== null) {
    window.clearTimeout(singleClickTimer);
    singleClickTimer = null;
  }
  if (missing.value) return;
  editingTitleValue.value = title.value;
  editingTitle.value = true;
  nextTick(() => {
    titleInputRef.value?.focus();
    titleInputRef.value?.select();
  });
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
  if (singleClickTimer !== null) {
    window.clearTimeout(singleClickTimer);
    singleClickTimer = null;
  }
});
</script>

<style scoped>
.task-ref-node {
  margin: 0.25rem 0;
}

.task-ref-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.25rem 0.5rem;
  border-radius: 6px;
  transition: background-color 120ms ease;
}

.task-ref-row:hover {
  background: var(--surface-hover, rgba(0, 0, 0, 0.04));
}

/* Унифицированный Checkbox из @kosmos/visuals. Eden подсовывает свой
   brand orange через CSS var override — `#ff5c00` тот же, что у
   bullet/ordered list markers в редакторе (см. App.css → `::marker`,
   index.css → `--eden-accent-color`). */
.task-ref-row :deep(.kosmos-checkbox) {
  --kosmos-checkbox-accent: var(--eden-accent-color);
}

.task-ref-title {
  flex: 1 1 auto;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  line-height: 1.4;
  color: var(--foreground, #1a1a1a);
}

.is-completed .task-ref-title {
  text-decoration: line-through;
  color: var(--muted-foreground, #888);
}

.task-ref-title-empty {
  color: var(--muted-foreground, #888);
  font-style: italic;
}

.is-missing .task-ref-title {
  color: var(--destructive-foreground, #b14040);
  text-decoration: line-through;
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

.is-loading .task-ref-title {
  color: var(--muted-foreground, #888);
}
</style>
