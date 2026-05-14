<script setup lang="ts">
// Корневой компонент Arrancador-extension'а.
//
// Адаптация vs legacy `apps/arrancador/src-vue/App.vue`:
//   - legacy: <RouterView /> поверх Pinia + Vue Router (memory/hash history).
//   - extension: single LayoutPage без router'а — у extension renderer'а нет
//     deep-linking, native window navigation отсутствует, поэтому простой
//     mount компонента достаточен.

import LayoutPage from "./pages/LayoutPage.vue";
</script>

<template>
  <LayoutPage />
</template>
