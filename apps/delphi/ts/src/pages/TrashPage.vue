<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { Archive } from "lucide-vue-next";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import TodoRow from "@kepler/visuals/components/TodoRow.vue";

const store = useTodoStore();
const { todos } = storeToRefs(store);
const filtered = computed(() => filterTodos(SmartList.Trash, todos.value));

const confirmingEmpty = shallowRef(false);

async function handleEmptyTrash() {
  await store.emptyTrash();
  confirmingEmpty.value = false;
}
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <div
      class="mx-auto w-full max-w-(--bringhurst-wide) flex items-center justify-center gap-2.5 px-7 pb-3 pt-6"
    >
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
      <div v-if="filtered.length > 0" class="ml-auto">
        <template v-if="confirmingEmpty">
          <span class="mr-2 text-xs text-rose-400">Удалить навсегда?</span>
          <button
            type="button"
            class="rounded px-2 py-1 text-xs text-rose-400 transition-colors hover:bg-rose-500/10"
            @click="handleEmptyTrash"
          >
            Да
          </button>
          <button
            type="button"
            class="text-(--muted-foreground) ml-1 rounded px-2 py-1 text-xs transition-colors hover:bg-(--muted)"
            @click="confirmingEmpty = false"
          >
            Нет
          </button>
        </template>
        <button
          v-else
          type="button"
          class="text-(--muted-foreground) rounded px-2 py-1 text-xs transition-colors hover:text-rose-400"
          @click="confirmingEmpty = true"
        >
          Очистить корзину
        </button>
      </div>
    </div>

    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-(--bringhurst-wide) pb-20 pt-1">
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
