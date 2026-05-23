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
  <label class="kosmos-settings-search">
    <Search class="kosmos-settings-search__icon" :size="14" :stroke-width="2" aria-hidden="true" />
    <input
      class="kosmos-settings-search__input"
      :value="modelValue"
      :placeholder="placeholder"
      type="text"
      @input="onInput"
    />
  </label>
</template>

<style scoped>
.kosmos-settings-search {
  display: flex;
  width: 212px;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 4px;
  background: var(--settings-search-surface);
  color: #fff;
  cursor: text;
}

.kosmos-settings-search:focus-within {
  background: var(--settings-search-surface-focused);
}

.kosmos-settings-search__icon {
  flex-shrink: 0;
  color: var(--second-text-color);
}

.kosmos-settings-search__input {
  width: 100%;
  padding: 0;
  border: none;
  background: transparent;
  color: #fff;
  caret-color: #fff;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
  cursor: text;
  outline: none;
}

.kosmos-settings-search__input::placeholder {
  color: var(--second-text-color);
}
</style>
