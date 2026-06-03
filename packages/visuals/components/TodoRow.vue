<script setup lang="ts">
import { shallowRef, computed, useTemplateRef, nextTick, ref, watch } from "vue";
import { Calendar as CalendarIcon, DollarSign } from "@lucide/vue";
import ContextMenu from "./ContextMenu.vue";
import ContextMenuItem from "./ContextMenuItem.vue";
import DateChip from "./DateChip.vue";
import type { TodoRowItem, TodoDropPayload, TodoRowUpdate } from "./types";

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
  update: [patch: TodoRowUpdate];
  drop: [payload: TodoDropPayload];
}>();

const isCompleted = computed(() => props.todo.isCompleted || props.todo.isCancelled);

const expanded = shallowRef(false);
const titleDraft = shallowRef(props.todo.title);
const notesDraft = shallowRef(props.todo.notes ?? "");
const dateDraft = shallowRef<string | null>(props.todo.scheduledDate ?? null);
const billableDraft = shallowRef(Boolean(props.todo.billable));
const priceDraft = shallowRef(
  props.todo.price !== null && props.todo.price !== undefined ? String(props.todo.price) : "",
);

watch(
  () => props.todo,
  (next) => {
    if (expanded.value) return;
    titleDraft.value = next.title;
    notesDraft.value = next.notes ?? "";
    dateDraft.value = next.scheduledDate ?? null;
    billableDraft.value = Boolean(next.billable);
    priceDraft.value = next.price !== null && next.price !== undefined ? String(next.price) : "";
  },
  { deep: true },
);

const titleInputRef = useTemplateRef<HTMLInputElement>("titleInput");
const rowRef = ref<HTMLElement>();
const isDragging = shallowRef(false);
const dragSuppressClick = shallowRef(false);

function toggleExpand() {
  if (!props.editable || props.todo.isTrashed) return;
  expanded.value = !expanded.value;
  if (expanded.value) {
    titleDraft.value = props.todo.title;
    notesDraft.value = props.todo.notes ?? "";
    dateDraft.value = props.todo.scheduledDate ?? null;
    billableDraft.value = Boolean(props.todo.billable);
    priceDraft.value =
      props.todo.price !== null && props.todo.price !== undefined ? String(props.todo.price) : "";
    nextTick(() => titleInputRef.value?.focus());
  }
}

function commitTitle() {
  const trimmed = titleDraft.value.trim();
  if (trimmed && trimmed !== props.todo.title) {
    emit("update", { title: trimmed });
  } else {
    titleDraft.value = props.todo.title;
  }
}

function commitNotes() {
  const next = notesDraft.value.trim() || null;
  if (next !== (props.todo.notes ?? null)) {
    emit("update", { notes: next });
  }
}

function commitDate(next: string | null) {
  dateDraft.value = next;
  if (next !== (props.todo.scheduledDate ?? null)) {
    emit("update", { scheduledDate: next });
  }
}

function toggleBillable() {
  billableDraft.value = !billableDraft.value;
  emit("update", { billable: billableDraft.value });
}

function commitPrice() {
  const trimmed = priceDraft.value.trim();
  const parsed = trimmed === "" ? null : Number(trimmed);
  const next = parsed !== null && Number.isFinite(parsed) ? parsed : null;
  if (next !== (props.todo.price ?? null)) {
    emit("update", { price: next });
  }
}

function onExpandKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    expanded.value = false;
  }
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

const menuOpen = shallowRef(false);
const menuX = shallowRef(0);
const menuY = shallowRef(0);

function onContextMenu(e: MouseEvent) {
  if (!props.editable) return;
  e.preventDefault();
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  menuOpen.value = true;
}

function closeMenu() {
  menuOpen.value = false;
}

function handleDelete() {
  closeMenu();
  expanded.value = false;
  emit("trash");
}

// ---------------------------------------------------------------------------
// Drag & Drop
// ---------------------------------------------------------------------------

let clone: HTMLElement | null = null;
let ghost: HTMLElement | null = null;
let startX = 0;
let startY = 0;
let lastTarget: TodoDropPayload | null = null;

let rowSnapshot: { id: string; top: number; bottom: number; mid: number }[] = [];

function onRowPointerDown(e: PointerEvent) {
  if (!props.draggable || expanded.value || e.button !== 0) return;

  const ox = e.clientX;
  const oy = e.clientY;
  let started = false;

  const onMove = (me: PointerEvent) => {
    if (!started && (Math.abs(me.clientX - ox) > 4 || Math.abs(me.clientY - oy) > 4)) {
      started = true;
      dragSuppressClick.value = true;
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

function onRowClick(e: MouseEvent) {
  if (dragSuppressClick.value) {
    dragSuppressClick.value = false;
    return;
  }
  if ((e.target as HTMLElement | null)?.closest("[data-stop-toggle]")) return;
  toggleExpand();
}

function beginDrag(cx: number, cy: number) {
  if (!rowRef.value) return;

  const rect = rowRef.value.getBoundingClientRect();
  startX = cx;
  startY = cy;
  lastTarget = null;

  rowSnapshot = getSiblingRows().map((el) => {
    const r = el.getBoundingClientRect();
    return { id: el.dataset.todoId!, top: r.top, bottom: r.bottom, mid: r.top + r.height / 2 };
  });

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

  ghost = document.createElement("div");
  ghost.dataset.dropGhost = "";
  Object.assign(ghost.style, {
    height: `${rect.height}px`,
    flex: "0 0 auto",
    borderRadius: "var(--radius)",
    border: "1px dashed color-mix(in srgb, var(--ring) 70%, transparent)",
    backgroundColor: "color-mix(in srgb, var(--secondary) 65%, transparent)",
    boxSizing: "border-box",
    pointerEvents: "none",
  });
  rowRef.value.parentElement?.insertBefore(ghost, rowRef.value);
  rowRef.value.style.display = "none";

  isDragging.value = true;
}

function getSiblingRows(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-todo-id]")).filter(
    (el) => el.dataset.todoId !== props.todo.id && !el.hasAttribute("data-drag-clone"),
  );
}

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
  if (!targetEl) return;

  const refNode = target.after ? targetEl.nextSibling : targetEl;
  const parent = targetEl.parentElement;
  if (!parent) return;
  if (ghost.parentElement === parent && ghost.nextSibling === refNode) return;
  parent.insertBefore(ghost, refNode as Node | null);
}

function onDragMove(e: PointerEvent) {
  if (!clone) return;

  const dx = e.clientX - startX;
  const dy = e.clientY - startY;
  clone.style.translate = `${dx}px ${dy}px`;
  clone.style.rotate = `${dx >= 0 ? 2.5 : -2.5}deg`;

  const target = findDropTarget(e.clientY);
  if (!target) return;

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

  rowRef.value?.style.removeProperty("display");
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
      'group relative flex flex-col rounded-(--radius) [corner-shape:var(--corner-shape)] px-8 will-change-transform hover:bg-(--secondary)',
      isDragging ? 'pointer-events-none opacity-0' : '',
      expanded ? 'bg-(--secondary)' : '',
    ]"
    @pointerdown="onRowPointerDown"
    @click="onRowClick"
    @contextmenu="onContextMenu"
  >
    <!-- Row header -->
    <div
      :class="[
        'flex h-10 items-center gap-4',
        draggable && !expanded ? 'cursor-grab active:cursor-grabbing' : 'cursor-pointer',
      ]"
    >
      <!-- Checkbox -->
      <button
        type="button"
        class="group/check shrink-0"
        data-stop-toggle
        @pointerdown.stop
        @click.stop="emit('complete')"
      >
        <span
          :class="[
            'relative block size-6 rounded-md border-2 border-(--ring) transition-colors group-hover/check:border-(--accent)',
            isCompleted ? 'border-(--accent)' : '',
          ]"
        >
          <span v-if="isCompleted" class="absolute inset-1 block rounded-sm bg-(--accent)" />
        </span>
      </button>

      <!-- Title (collapsed) -->
      <div class="min-w-0 flex-1">
        <div
          :class="[
            'truncate px-2 py-2 text-sm leading-5 select-none',
            isCompleted ? 'text-(--muted-foreground) line-through' : 'text-(--foreground)',
          ]"
        >
          {{ todo.title }}
        </div>
        <div
          v-if="!expanded && todo.notes"
          class="truncate text-xs text-(--muted-foreground)/70 select-none"
        >
          {{ todo.notes }}
        </div>
      </div>

      <!-- Trailing chips (collapsed) -->
      <div v-if="!expanded" class="flex items-center gap-2 text-xs text-(--muted-foreground)">
        <span
          v-if="todo.billable"
          class="flex h-6 items-center gap-2 rounded-full bg-emerald-500/15 px-2 text-emerald-500"
        >
          <DollarSign :size="16" />
          <span v-if="todo.price">{{ todo.price }}</span>
        </span>
        <span
          v-if="todo.scheduledDate"
          class="flex h-6 items-center gap-2 rounded-full bg-(--secondary) px-2"
        >
          <CalendarIcon :size="16" />
          {{ todo.scheduledDate }}
        </span>
      </div>

      <slot />
    </div>

    <!-- Expanded editor -->
    <div
      v-if="expanded"
      class="flex flex-col gap-2 py-4 pl-8 pr-2"
      data-stop-toggle
      @click.stop
      @pointerdown.stop
      @keydown="onExpandKeyDown"
    >
      <input
        ref="titleInput"
        type="text"
        :value="titleDraft"
        class="h-8 w-full rounded bg-(--secondary) px-2 text-sm font-medium text-(--foreground) outline-none ring-1 ring-(--ring) focus:ring-(--accent)"
        @input="titleDraft = ($event.target as HTMLInputElement).value"
        @blur="commitTitle"
        @keydown.enter.prevent="commitTitle"
      />

      <textarea
        :value="notesDraft"
        :rows="2"
        placeholder="Заметки"
        class="w-full resize-none rounded bg-(--secondary) p-2 text-xs text-(--muted-foreground) outline-none ring-1 ring-(--ring)/40 focus:ring-(--accent)"
        @input="notesDraft = ($event.target as HTMLTextAreaElement).value"
        @blur="commitNotes"
      />

      <div class="flex flex-wrap items-center gap-2">
        <DateChip :value="dateDraft" placeholder="Без даты" @update:value="commitDate" />

        <button
          type="button"
          :class="[
            'flex h-8 items-center gap-2 rounded-full px-4 text-xs transition-colors',
            billableDraft
              ? 'bg-emerald-500/15 text-emerald-500'
              : 'bg-(--secondary) text-(--muted-foreground) hover:bg-(--surface)',
          ]"
          @click="toggleBillable"
        >
          <DollarSign :size="16" />
          <span>Оплачиваемая</span>
        </button>

        <label
          v-if="billableDraft"
          class="flex h-8 items-center gap-2 rounded-full bg-(--secondary) px-4 text-xs"
        >
          <input
            type="number"
            inputmode="decimal"
            min="0"
            step="0.01"
            placeholder="Цена"
            :value="priceDraft"
            class="w-20 bg-transparent text-xs outline-none placeholder:text-(--muted-foreground)/60"
            @input="priceDraft = ($event.target as HTMLInputElement).value"
            @blur="commitPrice"
          />
        </label>
      </div>
    </div>
  </div>

  <ContextMenu :open="menuOpen" :x="menuX" :y="menuY" @close="closeMenu">
    <ContextMenuItem destructive @click="handleDelete"> Удалить </ContextMenuItem>
  </ContextMenu>
</template>
