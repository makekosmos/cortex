<script setup lang="ts">
import { computed } from "vue";
import { Book } from "lucide-vue-next";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import TodoRow from "@kepler/visuals/components/TodoRow.vue";

const store = useTodoStore();
const { todos } = storeToRefs(store);
const filtered = computed(() => filterTodos(SmartList.Logbook, todos.value));
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <div
      class="mx-auto w-full max-w-(--bringhurst-wide) flex items-center justify-center gap-2.5 px-7 pb-3 pt-6"
    >
      <Book :size="24" class="text-green-500" />
      <h1 class="text-2xl font-bold text-(--foreground) select-none">Журнал</h1>
      <span
        v-if="filtered.length > 0"
        class="text-sm text-(--muted-foreground)"
      >
        {{ filtered.length }}
      </span>
    </div>

    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-(--bringhurst-wide) pb-20 pt-1">
        <div
          v-if="filtered.length === 0"
          class="px-7 py-10 text-center text-sm text-(--muted-foreground)/60"
        >
          Завершённых задач нет
        </div>
        <div v-else class="flex flex-col">
          <TodoRow
            v-for="todo in filtered"
            :key="todo.id"
            :todo="todo"
            @complete="store.incompleteTodo(todo.id)"
          />
        </div>
      </div>
    </div>
  </div>
</template>
