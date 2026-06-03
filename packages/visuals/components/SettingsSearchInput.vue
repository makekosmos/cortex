<script setup lang="ts">
import { Search } from "@lucide/vue";

interface Props {
  modelValue: string;
  placeholder?: string;
}

withDefaults(defineProps<Props>(), {
  placeholder: "Поиск",
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

function onInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emit("update:modelValue", target.value);
}
</script>

<template>
  <label
    class="flex w-full cursor-text items-center gap-2 rounded bg-[var(--settings-search-surface)] px-2 py-2 text-xs text-white focus-within:bg-[var(--settings-search-surface-focused)]"
  >
    <Search
      class="shrink-0 text-[var(--second-text-color)]"
      :size="14"
      :stroke-width="2"
      aria-hidden="true"
    />
    <input
      class="w-full cursor-text border-0 bg-transparent p-0 font-[var(--font-sans)] text-[inherit] leading-[1.4] font-medium text-white caret-white outline-none placeholder:text-[var(--second-text-color)]"
      :value="modelValue"
      :placeholder="placeholder"
      type="text"
      @input="onInput"
    />
  </label>
</template>
