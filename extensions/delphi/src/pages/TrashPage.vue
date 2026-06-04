<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { SmartList } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { filterTodos } from "@/services/filters/todoFilterService";
import { Button, EmptyState, TodoRow } from "@kosmos/visuals";
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
        <h1 :class="[titleClass, 'text-2xl font-bold text-(--foreground) select-none']">Корзина</h1>
        <span v-if="filtered.length > 0" class="text-sm text-(--muted-foreground) select-none">
          {{ filtered.length }}
        </span>
      </div>
      <div v-if="filtered.length > 0" class="ml-auto">
        <template v-if="confirmingEmpty">
          <span class="mr-2 text-xs text-[var(--destructive)] select-none">
            Удалить навсегда?
          </span>
          <Button type="button" size="sm" variant="danger" @click="handleEmptyTrash"> Да </Button>
          <Button
            type="button"
            class="ml-1"
            size="sm"
            variant="ghost"
            @click="confirmingEmpty = false"
          >
            Нет
          </Button>
        </template>
        <Button v-else type="button" size="sm" variant="danger" @click="confirmingEmpty = true">
          Очистить корзину
        </Button>
      </div>
    </div>

    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div :class="[wrapClass, 'pb-20 pt-1']" :style="wrapStyle">
        <EmptyState v-if="filtered.length === 0" compact title="Корзина пуста" />
        <div v-else class="flex flex-col">
          <TodoRow v-for="todo in filtered" :key="todo.id" :todo="todo">
            <Button type="button" size="sm" variant="ghost" @click="store.restoreTodo(todo.id)">
              Восстановить
            </Button>
          </TodoRow>
        </div>
      </div>
    </div>
  </div>
</template>
