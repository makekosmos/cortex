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

export interface TodoDropPayload {
  targetId: string;
  after: boolean;
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
  drop: [payload: TodoDropPayload];
}>();

const isCompleted = computed(
  () => props.todo.isCompleted || props.todo.isCancelled,
);

const editing = shallowRef(false);
const draft = shallowRef(props.todo.title);
const inputRef = useTemplateRef<HTMLInputElement>("editInput");
const rowRef = ref<HTMLElement>();
const isDragging = shallowRef(false);

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

// ---------------------------------------------------------------------------
// Drag & Drop
// ---------------------------------------------------------------------------

const ROW_HEIGHT = 40;

let clone: HTMLElement | null = null;
let ghost: HTMLElement | null = null;
let startX = 0;
let startY = 0;
let lastTarget: TodoDropPayload | null = null;

// Snapshot of row positions taken once at drag start (before ghost distorts layout)
let rowSnapshot: { id: string; top: number; bottom: number; mid: number }[] = [];

function onRowPointerDown(e: PointerEvent) {
  if (!props.draggable || editing.value) return;

  const ox = e.clientX;
  const oy = e.clientY;
  let started = false;

  const onMove = (me: PointerEvent) => {
    if (!started && (Math.abs(me.clientX - ox) > 4 || Math.abs(me.clientY - oy) > 4)) {
      started = true;
      beginDrag(ox, oy);
    }
    if (started) onDragMove(me);
  };
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    if (started) onDragEnd();
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp, { once: true });
}

function beginDrag(cx: number, cy: number) {
  if (!rowRef.value) return;

  const rect = rowRef.value.getBoundingClientRect();
  startX = cx;
  startY = cy;
  lastTarget = null;

  // Snapshot sibling positions BEFORE any DOM changes
  rowSnapshot = getSiblingRows().map((el) => {
    const r = el.getBoundingClientRect();
    return { id: el.dataset.todoId!, top: r.top, bottom: r.bottom, mid: r.top + r.height / 2 };
  });

  // Floating clone
  clone = rowRef.value.cloneNode(true) as HTMLElement;
  clone.dataset.dragClone = "";
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
    opacity: "0.9",
  });
  document.body.appendChild(clone);
  document.body.style.cursor = "grabbing";

  // Ghost placeholder
  ghost = document.createElement("div");
  ghost.dataset.dropGhost = "";
  Object.assign(ghost.style, {
    height: "0px",
    overflow: "hidden",
    transition: "height 0.15s ease",
    borderRadius: "var(--radius)",
    backgroundColor: "var(--surface)",
    pointerEvents: "none",
  });

  isDragging.value = true;
}

function getSiblingRows(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>("[data-todo-id]"),
  ).filter(
    (el) =>
      el.dataset.todoId !== props.todo.id &&
      !el.hasAttribute("data-drag-clone"),
  );
}

/** Use the frozen snapshot to find the drop target — immune to ghost layout shifts. */
function findDropTarget(clientY: number): TodoDropPayload | null {
  if (rowSnapshot.length === 0) return null;

  for (const snap of rowSnapshot) {
    if (clientY < snap.mid) {
      return { targetId: snap.id, after: false };
    }
  }

  return { targetId: rowSnapshot[rowSnapshot.length - 1].id, after: true };
}

function positionGhost(target: TodoDropPayload) {
  if (!ghost) return;

  const rows = getSiblingRows();
  const targetEl = rows.find((el) => el.dataset.todoId === target.targetId);
  if (!targetEl?.parentElement) return;

  const refNode = target.after ? targetEl.nextSibling : targetEl;
  const parent = targetEl.parentElement;

  // Only move if position actually changed
  if (ghost.parentElement === parent && ghost.nextSibling === refNode) return;

  // Remove from old position
  if (ghost.parentElement) {
    ghost.style.transition = "none";
    ghost.style.height = "0px";
    ghost.parentElement.removeChild(ghost);
  }

  // Insert at new position with height animation
  parent.insertBefore(ghost, refNode as Node | null);
  ghost.offsetHeight; // force reflow
  ghost.style.transition = "height 0.15s ease";
  ghost.style.height = `${ROW_HEIGHT}px`;
}

function onDragMove(e: PointerEvent) {
  if (!clone) return;

  const dx = e.clientX - startX;
  const dy = e.clientY - startY;
  clone.style.translate = `${dx}px ${dy}px`;
  clone.style.rotate = `${dx >= 0 ? 2.5 : -2.5}deg`;

  const target = findDropTarget(e.clientY);
  if (!target) return;

  // Only update ghost if target changed
  if (!lastTarget || lastTarget.targetId !== target.targetId || lastTarget.after !== target.after) {
    lastTarget = target;
    positionGhost(target);
  }
}

function onDragEnd() {
  document.body.style.cursor = "";

  if (clone?.parentElement) {
    clone.parentElement.removeChild(clone);
    clone = null;
  }

  if (ghost?.parentElement) {
    ghost.parentElement.removeChild(ghost);
  }
  ghost = null;

  isDragging.value = false;
  rowSnapshot = [];

  if (lastTarget) {
    const payload = lastTarget;
    lastTarget = null;
    emit("drop", payload);
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
@pointerdown="onRowPointerDown"
    @dblclick="editable ? startEditing() : undefined"
  >

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
  border-radius: 6px;
  border: 2px solid var(--ring);
  transition: border-color 0.15s, background-color 0.15s;
  position: relative;
}

.check-box--done {
  border-color: var(--accent);
}

.check-box__inner {
  display: block;
  position: absolute;
  inset: 2px;
  border-radius: 3px;
  background-color: var(--accent);
}

.check-btn:hover .check-box:not(.check-box--done) {
  border-color: var(--accent);
}

.check-btn:hover .check-box--done {
  border-color: var(--accent);
}

.check-btn:hover .check-box--done .check-box__inner {
  background-color: var(--accent);
}

.todo-row {
  position: relative;
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
