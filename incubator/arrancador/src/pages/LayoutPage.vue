<script setup lang="ts">
// LayoutPage — корневой shell через актуальный @kosmos/visuals chrome.
import { computed, ref } from "vue";
import { PanelLeft } from "@lucide/vue";
import {
  DesktopChrome,
  DesktopContentSurface,
  TitlebarButton,
  type TitlebarPlatform,
} from "@kosmos/visuals";

import AppSidebar from "../components/AppSidebar.vue";
import AppSpotlight from "../components/AppSpotlight.vue";
import { useSearchQuery } from "../composables/useSearchQuery";

const sidebarHidden = ref(false);
const search = useSearchQuery();

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});
</script>

<template>
  <DesktopChrome class="arrancador-shell" appearance="settings" :platform="chromePlatform">
    <template #titlebar-leading>
      <TitlebarButton
        v-if="sidebarHidden"
        :title="sidebarHidden ? 'Показать сайдбар' : 'Скрыть сайдбар'"
        aria-label="Показать сайдбар"
        aria-pressed="false"
        data-testid="arrancador-titlebar-sidebar-toggle"
        @click="sidebarHidden = !sidebarHidden"
      >
        <PanelLeft :size="16" />
      </TitlebarButton>
    </template>

    <template #titlebar-trailing>
      <AppSpotlight v-model="search" />
    </template>

    <template v-if="!sidebarHidden" #sidebar>
      <AppSidebar :hidden="false" @update:hidden="(val) => (sidebarHidden = val)" />
    </template>

    <DesktopContentSurface padding-top="0" padding-inline="0" padding-bottom="0">
      <router-view />
    </DesktopContentSurface>
  </DesktopChrome>
</template>
