<script setup lang="ts">
import { computed } from "vue";
import { Archive } from "lucide-vue-next";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import TodoRow from "@/components/TodoRow.vue";

const store = useTodoStore();
const { todos } = storeToRefs(store);
const filtered = computed(() => filterTodos(SmartList.Trash, todos.value));
</script>

<template>
  <div class="flex w-full min-w-0 flex-col">
    <div class="flex items-center gap-2.5 px-7 pb-3 pt-6">
      <Archive :size="24" class="text-gray-500" />
      <h1 class="text-2xl font-bold text-(--foreground) select-none">
        Корзина
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
          Корзина пуста
        </div>
        <div v-else class="flex flex-col">
          <TodoRow v-for="todo in filtered" :key="todo.id" :todo="todo">
            <button
              type="button"
              class="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-emerald-500/15 hover:text-emerald-500"
              @click="store.restoreTodo(todo.id)"
            >
              Восстановить
            </button>
          </TodoRow>
        </div>
      </div>
    </div>
  </div>
</template>
