<script setup lang="ts">
import { shallowRef, computed, useTemplateRef, nextTick, ref } from "vue";
import { GripVertical } from "lucide-vue-next";

export interface TodoRowItem {
  id: string;
  title: string;
  notes?: string | null;
  isCompleted?: boolean;
  isCancelled?: boolean;
  isTrashed?: boolean;
}

const props = withDefaults(
  defineProps<{
    todo: TodoRowItem;
    draggable?: boolean;
    editable?: boolean;
  }>(),
  {
    draggable: true,
    editable: true,
  },
);

const emit = defineEmits<{
  complete: [];
  trash: [];
  rename: [newTitle: string];
  drop: [targetId: string];
}>();

const isCompleted = computed(
  () => props.todo.isCompleted || props.todo.isCancelled,
);

const editing = shallowRef(false);
const draft = shallowRef(props.todo.title);
const inputRef = useTemplateRef<HTMLInputElement>("editInput");
const rowRef = ref<HTMLElement>();
const isDragging = shallowRef(false);

let clone: HTMLElement | null = null;
let placeholderEl: HTMLElement | null = null;
let placeholderTimer: ReturnType<typeof setTimeout> | null = null;
let lastTargetId: string | null = null;
let lastInsertBeforeNode: Node | null = null;
let startX = 0;
let startY = 0;

function startEditing() {
  draft.value = props.todo.title;
  editing.value = true;
  nextTick(() => inputRef.value?.focus());
}

function commitEdit() {
  editing.value = false;
  const trimmed = draft.value.trim();
  if (trimmed && trimmed !== props.todo.title) {
    emit("rename", trimmed);
  }
}

function onEditKeyDown(e: KeyboardEvent) {
  if (e.key === "Enter") commitEdit();
  if (e.key === "Escape") editing.value = false;
}

function onRowPointerDown(e: PointerEvent) {
  if (!props.draggable || editing.value) return;

  const ox = e.clientX;
  const oy = e.clientY;
  let started = false;

  const onMove = (me: PointerEvent) => {
    if (!started && (Math.abs(me.clientX - ox) > 4 || Math.abs(me.clientY - oy) > 4)) {
      started = true;
      startDrag({ clientX: ox, clientY: oy, preventDefault: () => {} } as PointerEvent);
    }
  };
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp, { once: true });
}

function startDrag(e: PointerEvent) {
  if (!rowRef.value) return;
  e.preventDefault();

  const rect = rowRef.value.getBoundingClientRect();
  startX = e.clientX;
  startY = e.clientY;

  clone = rowRef.value.cloneNode(true) as HTMLElement;
  Object.assign(clone.style, {
    position: "fixed",
    top: `${rect.top}px`,
    left: `${rect.left}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    margin: "0",
    pointerEvents: "none",
    zIndex: "9999",
    backgroundColor: "var(--secondary)",
    translate: "0px 0px",
    rotate: "2.5deg",
    transition: "rotate 0.18s ease",
  });
  document.body.appendChild(clone);
  document.body.style.cursor = "grabbing";
  isDragging.value = true;

  document.addEventListener("pointermove", onPointerMove);
  document.addEventListener("pointerup", onPointerUp, { once: true });
}

function onPointerMove(e: PointerEvent) {
  if (!clone) return;

  const dx = e.clientX - startX;
  const dy = e.clientY - startY;
  const angle = dx >= 0 ? 2.5 : -2.5;
  clone.style.translate = `${dx}px ${dy}px`;
  clone.style.rotate = `${angle}deg`;

  updatePlaceholder(e);
}

function updatePlaceholder(e: PointerEvent) {
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const targetRow = el?.closest("[data-todo-id]") as HTMLElement | null;
  if (!targetRow || targetRow.dataset.todoId === props.todo.id) return;

  lastTargetId = targetRow.dataset.todoId ?? null;

  const rect = targetRow.getBoundingClientRect();
  const insertBefore = e.clientY < rect.top + rect.height * 0.4;
  const insertBeforeNode = insertBefore ? targetRow : targetRow.nextSibling;

  if (insertBeforeNode === lastInsertBeforeNode) return;
  lastInsertBeforeNode = insertBeforeNode;

  if (placeholderTimer) clearTimeout(placeholderTimer);

  if (!placeholderEl) {
    placeholderEl = document.createElement("div");
    placeholderEl.style.height = "0px";
    placeholderEl.style.overflow = "hidden";
    placeholderEl.style.transition = "height 0.12s ease";
    placeholderEl.style.pointerEvents = "none";
    placeholderTimer = setTimeout(() => {
      if (!placeholderEl) return;
      targetRow.parentElement?.insertBefore(placeholderEl!, insertBeforeNode as Node | null);
      requestAnimationFrame(() => { if (placeholderEl) placeholderEl.style.height = "40px"; });
    }, 80);
  } else {
    placeholderEl.style.height = "0px";
    placeholderTimer = setTimeout(() => {
      if (!placeholderEl) return;
      targetRow.parentElement?.insertBefore(placeholderEl, insertBeforeNode as Node | null);
      requestAnimationFrame(() => { if (placeholderEl) placeholderEl.style.height = "40px"; });
    }, 120);
  }
}

function removePlaceholder() {
  if (placeholderTimer) clearTimeout(placeholderTimer);
  placeholderTimer = null;
  if (placeholderEl?.parentElement) {
    placeholderEl.parentElement.removeChild(placeholderEl);
  }
  placeholderEl = null;
  lastInsertBeforeNode = null;
}

function onPointerUp() {
  document.removeEventListener("pointermove", onPointerMove);
  document.body.style.cursor = "";

  const dropTargetId = lastTargetId;
  lastTargetId = null;

  removePlaceholder();

  if (clone) {
    document.body.removeChild(clone);
    clone = null;
  }
  isDragging.value = false;

  if (dropTargetId && dropTargetId !== props.todo.id) {
    emit("drop", dropTargetId);
  }
}
</script>

<template>
  <div
    ref="rowRef"
    :data-todo-id="todo.id"
    :class="[
      'todo-row group flex h-10 items-center gap-3 px-7 hover:bg-(--secondary)',
      isDragging ? 'opacity-0' : '',
      draggable && !editing ? 'cursor-grab active:cursor-grabbing' : 'cursor-pointer',
    ]"
    style="max-width: var(--bringhurst-wide)"
    @pointerdown="onRowPointerDown"
    @dblclick="editable ? startEditing() : undefined"
  >
    <!-- Drag handle (visual indicator only) -->
    <GripVertical
      v-if="draggable"
      :size="14"
      class="shrink-0 text-(--muted-foreground)/30 opacity-0 transition-opacity group-hover:opacity-100"
    />

    <!-- Checkbox -->
    <button type="button" class="check-btn shrink-0" @pointerdown.stop @click.stop="emit('complete')">
      <span :class="['check-box', isCompleted ? 'check-box--done' : '']">
        <span v-if="isCompleted" class="check-box__inner" />
      </span>
    </button>

    <!-- Content -->
    <div class="min-w-0 flex-1">
      <input
        v-if="editing"
        ref="editInput"
        :value="draft"
        class="block w-full rounded bg-(--secondary) px-1 py-0.5 text-sm leading-5 text-(--foreground) outline-none ring-1 ring-(--ring)"
        @input="draft = ($event.target as HTMLInputElement).value"
        @blur="commitEdit"
        @keydown="onEditKeyDown"
      />
      <template v-else>
        <div
          :class="[
            'truncate text-sm leading-5 px-1 py-0.5 select-none',
            isCompleted
              ? 'text-(--muted-foreground) line-through'
              : 'text-(--foreground)',
          ]"
        >
          {{ todo.title }}
        </div>
        <div
          v-if="todo.notes"
          class="truncate text-xs text-(--muted-foreground)/70 select-none"
        >
          {{ todo.notes }}
        </div>
      </template>
    </div>

    <slot />

    <!-- Quick actions (visible on hover) -->
    <div
      v-if="editable && !editing && !todo.isTrashed"
      class="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100"
    >
      <button
        type="button"
        title="В корзину"
        class="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-red-500/15 hover:text-red-500 select-none"
        @pointerdown.stop
        @click.stop="emit('trash')"
      >
        Удалить
      </button>
    </div>
  </div>
</template>

<style scoped>
.check-box {
  display: block;
  width: 18px;
  height: 18px;
  border-radius: 5px;
  border: 2px solid var(--ring);
  transition: border-color 0.15s, background-color 0.15s;
  position: relative;
}

.check-box--done {
  border-color: #C13332;
}

.check-box__inner {
  display: block;
  position: absolute;
  inset: 2px;
  border-radius: 2px;
  background-color: #C13332;
}

.check-btn:hover .check-box:not(.check-box--done) {
  border-color: rgb(239 68 68);
}

.check-btn:hover .check-box--done {
  border-color: rgb(220 38 38);
}

.check-btn:hover .check-box--done .check-box__inner {
  background-color: rgb(220 38 38);
}

.todo-row {
  border-radius: var(--radius);
  corner-shape: var(--corner-shape);
  will-change: transform;
}

.todo-focus-pulse {
  animation: focus-pulse 1.2s ease;
}

@keyframes focus-pulse {
  0% { opacity: 1; }
  20% { opacity: 0.5; }
  100% { opacity: 1; }
}
</style>
