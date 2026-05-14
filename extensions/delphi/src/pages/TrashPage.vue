<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import { TodoRow } from "@kosmos/visuals";
import { useSidebarState } from "@/composables/useSidebarState";

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
      ref="titleWrapRef"
      :class="[wrapClass, 'flex min-h-8 items-center gap-2.5 px-7 pb-3 pt-6']"
      :style="wrapStyle"
    >
      <div ref="titleGroupRef" :class="titleGroupClass" :style="titleGroupStyle">
        <h1 :class="[titleClass, 'text-2xl font-bold text-(--foreground) select-none']">
          Корзина
        </h1>
        <span
          v-if="filtered.length > 0"
          class="text-sm text-(--muted-foreground) select-none"
        >
          {{ filtered.length }}
        </span>
      </div>
      <div v-if="filtered.length > 0" class="ml-auto">
        <template v-if="confirmingEmpty">
          <span class="mr-2 text-xs text-rose-400 select-none">Удалить навсегда?</span>
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
      <div :class="[wrapClass, 'pb-20 pt-1']" :style="wrapStyle">
        <div
          v-if="filtered.length === 0"
          class="px-7 py-10 text-center text-sm text-(--muted-foreground)/60"
        >
          <span class="select-none">Корзина пуста</span>
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
