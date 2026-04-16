<template>
  <div
    v-if="isOpen"
    class="search-overlay-backdrop"
    role="dialog"
    aria-modal="true"
    @click.self="emit('close')"
  >
    <div class="search-overlay" @keydown="handleKeyDown">
      <div class="search-overlay-input-wrap">
        <svg
          class="search-overlay-icon"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <input
          ref="inputRef"
          type="text"
          class="search-overlay-input"
          placeholder="Поиск..."
          :value="query"
          autofocus
          @input="emit('queryChange', ($event.target as HTMLInputElement).value)"
        />
        <button v-if="query" class="search-overlay-clear-btn" type="button" @click="emit('close')">
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
      <div class="search-overlay-results">
        <template v-if="query.trim()">
          <template v-if="results.length > 0">
            <div
              v-for="(result, index) in results"
              :key="`${result.entryId}-${index}`"
              class="search-overlay-result-item"
              :class="{ 'is-selected': selectedIndex === index }"
              @click="void emit('resultSelect', result.entryId)"
              @mouseenter="selectedIndex = index"
            >
              <div class="entry-title">
                {{ entryTitles[result.entryId] || "Без названия" }}
              </div>
              <div class="search-result-text">{{ result.text }}</div>
            </div>
          </template>
          <div v-else class="search-overlay-empty">Ничего не найдено</div>
        </template>
        <div v-else class="search-overlay-empty">Начните вводить для поиска</div>
      </div>
      <div class="search-overlay-footer">
        <div class="search-overlay-hint">
          <kbd>↑</kbd><kbd>↓</kbd>
          <span>навигация</span>
        </div>
        <div class="search-overlay-hint">
          <kbd>Enter</kbd>
          <span>открыть</span>
        </div>
        <div class="search-overlay-hint">
          <kbd>Esc</kbd>
          <span>закрыть</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { ref, watch } from "vue";
import "./SearchOverlay.css";

const props = defineProps<{
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

const inputRef = ref<HTMLInputElement | null>(null);
const selectedIndex = ref(0);

watch(
  () => props.isOpen,
  (open) => {
    if (open) {
      selectedIndex.value = 0;
      setTimeout(() => inputRef.value?.focus(), 0);
    }
  },
);

watch(
  () => props.results,
  () => {
    selectedIndex.value = 0;
  },
);

function handleKeyDown(event: KeyboardEvent) {
  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      selectedIndex.value = Math.min(selectedIndex.value + 1, props.results.length - 1);
      break;
    case "ArrowUp":
      event.preventDefault();
      selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
      break;
    case "Enter":
      event.preventDefault();
      if (props.results[selectedIndex.value]) {
        emit("resultSelect", props.results[selectedIndex.value].entryId);
      }
      break;
    case "Escape":
      event.preventDefault();
      emit("close");
      break;
  }
}
</script>
