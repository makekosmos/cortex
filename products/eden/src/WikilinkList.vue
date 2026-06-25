<template>
  <div class="wikilink-suggestions">
    <template v-if="items.length">
      <button
        v-for="(item, index) in items"
        :key="item.id"
        class="suggestion-item"
        :class="{ 'is-selected': index === selectedIndex }"
        @click="selectItem(index)"
      >
        {{ item.title }}
      </button>
    </template>
    <div v-else class="suggestion-item no-result">Ничего не найдено</div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";

const props = defineProps<{
  items: Array<{ id: string; title: string }>;
  command: (props: { id: string; label: string }) => void;
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
  if (item) props.command({ id: item.id, label: item.title });
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
