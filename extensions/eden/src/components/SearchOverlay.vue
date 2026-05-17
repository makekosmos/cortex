<template>
  <CommandPalette
    :open="isOpen"
    :query="query"
    placeholder="Поиск..."
    dialog-test-id="eden-search-palette"
    input-test-id="eden-search-input"
    @update:open="handleOpenChange"
    @update:query="emit('queryChange', $event)"
  >
    <template #default="{ query: activeQuery }">
      <div v-if="activeQuery.trim()" class="flex flex-col">
        <div v-if="results.length > 0" class="px-2 py-1">
          <button
            v-for="(result, index) in results"
            :key="`${result.entryId}-${index}`"
            data-cmd-item
            data-testid="eden-search-result-item"
            type="button"
            class="flex w-full flex-col items-start gap-1 rounded-lg px-3 py-3 text-left outline-none transition-colors hover:bg-(--surface) focus:bg-(--surface)"
            @click="emit('resultSelect', result.entryId)"
          >
            <div class="truncate text-sm font-medium text-(--foreground)">
              {{ entryTitles[result.entryId] ?? "Без названия" }}
            </div>
            <div class="line-clamp-2 text-xs text-(--muted-foreground)">
              {{ result.text }}
            </div>
          </button>
        </div>
        <EmptyState v-else title="Ничего не найдено" compact />
      </div>

      <EmptyState v-else title="Начните вводить для поиска" compact />

      <div class="mt-2 flex items-center justify-end gap-4 border-t border-(--border) px-4 py-3 text-xs text-(--muted-foreground)">
        <span><kbd>↑</kbd><kbd>↓</kbd> навигация</span>
        <span><kbd>Enter</kbd> открыть</span>
        <span><kbd>Esc</kbd> закрыть</span>
      </div>
    </template>
  </CommandPalette>
</template>

<script setup lang="ts">
import { CommandPalette, EmptyState } from "@kepler/visuals";

defineProps<{
  isOpen: boolean;
  query: string;
  results: SearchResult[];
  entryTitles: Record<string, string>;
}>();

const emit = defineEmits<{
  queryChange: [value: string];
  close: [];
  resultSelect: [entryId: string];
}>();

function handleOpenChange(open: boolean) {
  if (!open) {
    emit("close");
  }
}
</script>
