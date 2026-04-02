<script setup lang="ts">
import { computed } from "vue";
import { Inbox, Plus } from "lucide-vue-next";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import { useQuickEntry } from "@/composables/useQuickEntry";
import TodoRow from "@kepler/visuals/components/TodoRow.vue";

const { show: openQuickEntry } = useQuickEntry();

const store = useTodoStore();
const { todos } = storeToRefs(store);
const filtered = computed(() => filterTodos(SmartList.Inbox, todos.value));
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <div class="flex items-center gap-2.5 px-7 pb-3 pt-6">
      <Inbox :size="24" class="text-blue-500" />
      <h1 class="text-2xl font-bold text-(--foreground) select-none">
        Входящие
      </h1>
      <span
        v-if="filtered.length > 0"
        class="text-sm text-(--muted-foreground)"
      >
        {{ filtered.length }}
      </span>
    </div>

    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div class="pb-20 pt-1">
        <div
          v-if="filtered.length === 0"
          class="px-7 py-10 text-center text-sm text-(--muted-foreground)/60"
        >
          Нет входящих задач
        </div>
        <div v-else class="flex flex-col">
          <TodoRow
            v-for="todo in filtered"
            :key="todo.id"
            :todo="todo"
            @complete="store.completeTodo(todo.id)"
            @trash="store.trashTodo(todo.id)"
            @rename="store.updateTodo(todo.id, { title: $event })"
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
