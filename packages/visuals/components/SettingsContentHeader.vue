<script setup lang="ts">
// Тонкий drag-бар над контентом settings-страницы: back/forward chrome-кнопки
// (placeholder'ы, как в Kepler) + правый слот (advanced-toggle и т. п.).
// Стили приходят из общего `@kosmos/visuals/settings-shell.css` (классы
// `.content-header*` под `.settings-shell`). Используется и Kepler, и Eden.
import { ChevronLeft, ChevronRight } from "@lucide/vue";

withDefaults(
  defineProps<{
    /** Включить/выключить кнопки навигации (по умолчанию disabled-плейсхолдеры). */
    backDisabled?: boolean;
    forwardDisabled?: boolean;
  }>(),
  {
    backDisabled: true,
    forwardDisabled: true,
  },
);

const emit = defineEmits<{
  back: [];
  forward: [];
}>();
</script>

<template>
  <header class="content-header">
    <div class="content-header__nav">
      <button
        type="button"
        class="chrome-control"
        :disabled="backDisabled"
        title="Назад"
        aria-label="Назад"
        @click="emit('back')"
      >
        <ChevronLeft :size="14" :stroke-width="2" />
      </button>
      <button
        type="button"
        class="chrome-control"
        :disabled="forwardDisabled"
        title="Вперёд"
        aria-label="Вперёд"
        @click="emit('forward')"
      >
        <ChevronRight :size="14" :stroke-width="2" />
      </button>
    </div>

    <div class="content-header__right">
      <slot name="right" />
    </div>
  </header>
</template>
