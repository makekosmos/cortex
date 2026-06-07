<script setup lang="ts">
import { Minus, Plus } from "@lucide/vue";

const props = defineProps<{
  fontFamily: string;
  fontSize: number;
}>();

const emit = defineEmits<{
  updateFontFamily: [value: string];
  updateFontSize: [value: number];
}>();

const fontFamilies = ["Source Serif 4", "Georgia", "Palatino Linotype", "Inter", "Geist"];
</script>

<template>
  <div class="reader-toolbar">
    <select
      class="reader-toolbar__select"
      :value="fontFamily"
      aria-label="Шрифт"
      @change="emit('updateFontFamily', ($event.target as HTMLSelectElement).value)"
    >
      <option v-for="family in fontFamilies" :key="family" :value="family">
        {{ family }}
      </option>
    </select>

    <div class="reader-toolbar__stepper" aria-label="Размер текста">
      <button
        class="reader-toolbar__icon-btn"
        type="button"
        title="Уменьшить текст"
        :disabled="fontSize <= 14"
        @click="emit('updateFontSize', fontSize - 1)"
      >
        <Minus :size="16" aria-hidden="true" />
      </button>
      <span class="reader-toolbar__font-size">{{ fontSize }}</span>
      <button
        class="reader-toolbar__icon-btn"
        type="button"
        title="Увеличить текст"
        :disabled="fontSize >= 28"
        @click="emit('updateFontSize', fontSize + 1)"
      >
        <Plus :size="16" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
