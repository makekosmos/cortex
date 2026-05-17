<script setup lang="ts">
// LayoutPage — корневой shell (titlebar + sidebar + router outlet).
//
// Адаптация vs `apps/arrancador/src-vue/pages/LayoutPage.vue`:
//   - убраны Pinia store / RouterView с history-controls / Teleport mobile menu /
//     toast bridge / kosmos-desktop-chrome layout. Extension renderer не имеет
//     native window-controls и mobile breakpoint'ов.
//   - данные тянутся через `useGames()` composable (kepler.ark.request).
//   - Vue Router добавлен (createMemoryHistory) — sidebar теперь использует
//     <router-link>, а основной контент рендерится <router-view />.
import { ref } from "vue";

import AppSidebar from "../components/AppSidebar.vue";
import AppSpotlight from "../components/AppSpotlight.vue";
import AppTitlebar from "../components/AppTitlebar.vue";
import { useSearchQuery } from "../composables/useSearchQuery";

const sidebarHidden = ref(false);
const search = useSearchQuery();
</script>

<template>
  <div class="arrancador-shell">
    <AppTitlebar
      :sidebar-hidden="sidebarHidden"
      @toggle-sidebar="sidebarHidden = !sidebarHidden"
    >
      <template #right>
        <AppSpotlight v-model="search" />
      </template>
    </AppTitlebar>

    <div class="arrancador-shell__body">
      <AppSidebar
        :hidden="sidebarHidden"
        @update:hidden="(val) => (sidebarHidden = val)"
      />

      <div class="arrancador-content">
        <router-view />
      </div>
    </div>
  </div>
</template>
