<script setup lang="ts">
// Упрощённый sidebar Arrancador-extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/components/AppSidebar.vue`:
//   - legacy 302-строчный sidebar с resize / persistent-config / RouterLink /
//     i18n / sidebar-config Pinia store сведён к статичному списку разделов.
//   - "Каталог" / "Статистика" / "Сканер" / "Sqoba" / "Настройки" — UI остаются
//     placeholder'ами, full feature parity = Phase 5+.
//   - active-state управляется простым v-model'ом, без vue-router.

import type { ArrancadorSection } from "../pages/LayoutPage.vue";

defineProps<{
  current: ArrancadorSection;
  hidden: boolean;
}>();

const emit = defineEmits<{
  select: [section: ArrancadorSection];
}>();

const items: { id: ArrancadorSection; label: string }[] = [
  { id: "library", label: "Библиотека" },
  { id: "catalogue", label: "Каталог" },
  { id: "statistics", label: "Статистика" },
  { id: "settings", label: "Настройки" },
];

function select(section: ArrancadorSection) {
  emit("select", section);
}
</script>

<template>
  <nav v-if="!hidden" class="arrancador-sidebar" aria-label="Навигация Arrancador">
    <div class="arrancador-sidebar__heading">Разделы</div>
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="arrancador-sidebar__item"
      :class="{ 'arrancador-sidebar__item--active': current === item.id }"
      @click="select(item.id)"
    >
      {{ item.label }}
    </button>
  </nav>
</template>
