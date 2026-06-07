<template>
  <div class="slash-commands">
    <template v-if="items.length">
      <button
        v-for="(item, index) in items"
        :key="index"
        class="suggestion-item"
        :class="{ 'is-selected': index === selectedIndex }"
        @click="selectItem(index)"
      >
        <span class="command-icon">{{ item.icon }}</span>
        <div class="command-info">
          <span class="command-title">{{ item.title }}</span>
        </div>
      </button>
    </template>
    <div v-else class="suggestion-item no-result">Команд не найдено</div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import type { Editor, Range } from "@tiptap/vue-3";

export interface SlashCommandItem {
  title: string;
  description?: string;
  icon: string;
  command: (props: { editor: Editor; range: Range }) => void;
}

const props = defineProps<{
  items: SlashCommandItem[];
  command: (item: SlashCommandItem) => void;
}>();

const selectedIndex = ref(0);

watch(
  () => props.items,
  () => {
    selectedIndex.value = 0;
  },
);

function selectItem(index: number) {
  const item = props.items[index];
  if (item) props.command(item);
}

defineExpose({
  onKeyDown: ({ event }: { event: KeyboardEvent }): boolean => {
    if (event.key === "ArrowUp") {
      selectedIndex.value = (selectedIndex.value + props.items.length - 1) % props.items.length;
      return true;
    }
    if (event.key === "ArrowDown") {
      selectedIndex.value = (selectedIndex.value + 1) % props.items.length;
      return true;
    }
    if (event.key === "Enter") {
      selectItem(selectedIndex.value);
      return true;
    }
    return false;
  },
});
</script>
