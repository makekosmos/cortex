<script setup lang="ts">
// Упрощённый Spotlight / поиск для Arrancador-extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/components/AppSpotlight.vue`:
//   - legacy 370-строчный modal-spotlight с reka-ui Combobox / cmd-K shortcut /
//     RouterLink сведён к простому inline-input'у над библиотекой.
//   - filter — пользовательский фильтр текущего списка game_obj (LibraryPage
//     слушает v-model). Full spotlight UX (global cmd-k, fuzzy search across
//     types) проксируется через kepler-shell launcher, не через extension.

defineProps<{ modelValue: string }>();
defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div class="arrancador-spotlight">
    <input
      type="search"
      class="arrancador-spotlight__input"
      placeholder="Поиск по библиотеке"
      :value="modelValue"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
  </div>
</template>
