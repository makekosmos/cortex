<script setup lang="ts">
// Sidebar Arrancador-extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/components/AppSidebar.vue`:
//   - legacy 302-строчный sidebar (resize / persistent-config / i18n /
//     sidebar-config Pinia store) сведён к статичному списку router-link'ов.
//   - active-state определяется Vue Router'ом (`router-link-active`).

defineProps<{
  hidden: boolean;
}>();

const items = [
  { to: "/", label: "Библиотека" },
  { to: "/catalogue", label: "Каталог" },
  { to: "/scan", label: "Сканер" },
  { to: "/sqoba", label: "SQOBA" },
  { to: "/stats", label: "Статистика" },
  { to: "/settings", label: "Настройки" },
];
</script>

<template>
  <nav
    v-if="!hidden"
    class="arrancador-sidebar"
    aria-label="Навигация Arrancador"
  >
    <div class="arrancador-sidebar__heading">Разделы</div>
    <router-link
      v-for="item in items"
      :key="item.to"
      :to="item.to"
      class="arrancador-sidebar__item"
      exact-active-class="arrancador-sidebar__item--active"
    >
      {{ item.label }}
    </router-link>
  </nav>
</template>
