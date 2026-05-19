<script setup lang="ts">
// Упрощённая titlebar Arrancador'а для Kepler-shell extension'а.
//
// Window controls (minimize/maximize/close) рендерятся через
// `WindowControls` из `@kosmos/visuals` — компонент сам вызывает
// `window.kepler.window.*` bridge.

import { WindowControls } from "@kosmos/visuals";

defineProps<{
  sidebarHidden: boolean;
}>();

defineEmits<{
  toggleSidebar: [];
}>();
</script>

<template>
  <header class="arrancador-titlebar">
    <div class="arrancador-titlebar__left">
      <button
        type="button"
        class="arrancador-titlebar__btn"
        :aria-label="sidebarHidden ? 'Показать сайдбар' : 'Скрыть сайдбар'"
        @click="$emit('toggleSidebar')"
      >
        <!-- Простая иконка-меню без зависимости от lucide-vue-next.
             Legacy импортирует Menu / X из lucide, здесь оставляем inline SVG
             чтобы не тянуть тяжёлую icon-библиотеку в extension bundle. -->
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
          <line x1="3" y1="6" x2="21" y2="6" />
          <line x1="3" y1="12" x2="21" y2="12" />
          <line x1="3" y1="18" x2="21" y2="18" />
        </svg>
      </button>
      <span class="arrancador-titlebar__title">Arrancador</span>
    </div>
    <div class="arrancador-titlebar__right">
      <slot name="right" />
      <WindowControls />
    </div>
  </header>
</template>
