<script setup lang="ts">
// Упрощённая titlebar Arrancador'а для Kepler-shell extension'а.
//
// TODO: рассмотреть миграцию на `Titlebar` / `DesktopChrome` из
// `@kepler/visuals` после того как extension'у понадобятся history-controls
// или native window-buttons. Сейчас visuals.Titlebar не покрывает наш
// частный case (sidebar-toggle слева + slot для AppSpotlight справа без
// back/forward), поэтому держим custom вариант.
//
// Адаптация vs `apps/arrancador/src-vue/components/AppTitlebar.vue`:
//   - убраны зависимости от Vue Router (back/forward — placeholder no-op).
//   - убраны вызовы `window.electronAPI.windowControls.*` — kepler-shell
//     управляет окном через extension-host bridge; titlebar остаётся
//     drag-handle'ом плюс отображает название приложения.
//   - native window-controls (minimize/maximize/close) пока не пробрасываются
//     отдельным IPC; shell отрисует системный chrome или добавит controls сам.

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
    </div>
  </header>
</template>
