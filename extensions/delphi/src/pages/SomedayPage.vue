<script setup lang="ts">
import { computed } from "vue";
import { Plus } from "@lucide/vue";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import { useQuickEntry } from "@/composables/useQuickEntry";
import { useSidebarState } from "@/composables/useSidebarState";
import { TodoRow } from "@kosmos/visuals";

const { show: openQuickEntry } = useQuickEntry();
const {
  wrapClass,
  wrapStyle,
  titleWrapRef,
  titleGroupRef,
  titleGroupClass,
  titleGroupStyle,
  titleClass,
} = useSidebarState();

const store = useTodoStore();
const { todos } = storeToRefs(store);
const filtered = computed(() => filterTodos(SmartList.Someday, todos.value));

function handleDrop(payload: { targetId: string; after: boolean }, sourceId: string) {
  if (sourceId === payload.targetId) return;
  const list = [...filtered.value];
  const srcIdx = list.findIndex((t) => t.id === sourceId);
  if (srcIdx === -1) return;
  const [item] = list.splice(srcIdx, 1);
  const tgtIdx = list.findIndex((t) => t.id === payload.targetId);
  if (tgtIdx === -1) return;
  list.splice(payload.after ? tgtIdx + 1 : tgtIdx, 0, item);
  list.forEach((t, i) => store.updateTodo(t.id, { sortOrder: i }));
}
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <div
      ref="titleWrapRef"
      :class="[wrapClass, 'flex min-h-8 items-center gap-2.5 px-7 pb-3 pt-6']"
      :style="wrapStyle"
    >
      <div ref="titleGroupRef" :class="titleGroupClass" :style="titleGroupStyle">
        <h1 :class="[titleClass, 'text-2xl font-bold text-(--foreground) select-none']">
          Когда-нибудь
        </h1>
        <span v-if="filtered.length > 0" class="text-sm text-(--muted-foreground) select-none">
          {{ filtered.length }}
        </span>
      </div>
    </div>

    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div :class="[wrapClass, 'pb-20 pt-1']" :style="wrapStyle">
        <div
          v-if="filtered.length === 0"
          class="px-7 py-10 text-center text-sm text-(--muted-foreground)/60"
        >
          <span class="select-none">Пока нет отложенных задач</span>
        </div>
        <div v-else class="flex flex-col">
          <TodoRow
            v-for="todo in filtered"
            :key="todo.id"
            :todo="todo"
            @complete="store.completeTodo(todo.id)"
            @trash="store.trashTodo(todo.id)"
            @update="store.updateTodo(todo.id, $event)"
            @drop="handleDrop($event, todo.id)"
          />
        </div>
      </div>
    </div>

    <button
      type="button"
      class="absolute bottom-6 right-6 flex h-12 w-12 items-center justify-center rounded-full bg-(--primary) text-(--primary-foreground) shadow-lg transition-transform hover:scale-105 active:scale-95"
      title="Новая задача (⌘N)"
      @click="openQuickEntry"
    >
      <Plus :size="24" />
    </button>
  </div>
</template>
