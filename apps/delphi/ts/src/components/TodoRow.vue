<script setup lang="ts">
import { shallowRef, computed, useTemplateRef, nextTick } from "vue";
import { Circle, CheckCircle2 } from "lucide-vue-next";
import type { TodoItem } from "@/types/task";

const props = defineProps<{
  todo: TodoItem;
}>();

const emit = defineEmits<{
  complete: [];
  trash: [];
  rename: [newTitle: string];
}>();

const isCompleted = computed(
  () => props.todo.isCompleted || props.todo.isCancelled,
);

const editing = shallowRef(false);
const draft = shallowRef(props.todo.title);
const inputRef = useTemplateRef<HTMLInputElement>("editInput");

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
</script>

<template>
  <div
    class="group flex items-center gap-3 px-7 py-2 hover:bg-(--secondary)"
    @dblclick="startEditing"
  >
    <!-- Checkbox -->
    <button type="button" class="shrink-0" @click="emit('complete')">
      <CheckCircle2
        v-if="isCompleted"
        :size="18"
        class="text-(--muted-foreground)"
      />
      <Circle
        v-else
        :size="18"
        class="text-(--ring) hover:text-(--foreground)"
      />
    </button>

    <!-- Content -->
    <div class="min-w-0 flex-1">
      <input
        v-if="editing"
        ref="editInput"
        :value="draft"
        class="w-full rounded bg-(--secondary) px-1 py-0.5 text-sm text-(--foreground) outline-none ring-1 ring-(--ring)"
        @input="draft = ($event.target as HTMLInputElement).value"
        @blur="commitEdit"
        @keydown="onEditKeyDown"
      />
      <template v-else>
        <div
          :class="[
            'truncate text-sm',
            isCompleted
              ? 'text-(--muted-foreground) line-through'
              : 'text-(--foreground)',
          ]"
        >
          {{ todo.title }}
        </div>
        <div
          v-if="todo.notes"
          class="truncate text-xs text-(--muted-foreground)/70"
        >
          {{ todo.notes }}
        </div>
      </template>
    </div>

    <slot />

    <!-- Quick actions (visible on hover) -->
    <div
      v-if="!editing && !todo.isTrashed"
      class="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100"
    >
      <button
        type="button"
        title="В корзину"
        class="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-red-500/15 hover:text-red-500"
        @click="emit('trash')"
      >
        Удалить
      </button>
    </div>
  </div>
</template>
