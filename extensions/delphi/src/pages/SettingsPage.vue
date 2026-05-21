<script setup lang="ts">
import { computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import GeneralSettingsTab from "@/components/settings/GeneralSettingsTab.vue";
import SpacesSettingsTab from "@/components/settings/SpacesSettingsTab.vue";

type SettingsTab = "general" | "spaces";

const route = useRoute();
const router = useRouter();
const isMac = navigator.platform.startsWith("Mac");

const settingsPageClasses = computed(() => [
  "delphi-settings-page",
  { "delphi-settings-page--mac": isMac },
]);

function normalizeSettingsTab(rawTab: unknown): SettingsTab {
  return rawTab === "spaces" ? "spaces" : "general";
}

const activeTab = computed<SettingsTab>(() =>
  normalizeSettingsTab(Array.isArray(route.query.tab) ? route.query.tab[0] : route.query.tab),
);

watch(
  () => route.query.tab,
  (rawTab) => {
    const tabValue = Array.isArray(rawTab) ? rawTab[0] : rawTab;
    const normalizedTab = normalizeSettingsTab(tabValue);

    if (tabValue === normalizedTab) return;

    void router.replace({
      query: {
        ...route.query,
        tab: normalizedTab,
      },
    });
  },
  { immediate: true },
);
</script>

<template>
  <section :class="settingsPageClasses">
    <div class="delphi-settings-content-inner">
      <GeneralSettingsTab v-if="activeTab === 'general'" />
      <SpacesSettingsTab v-else />
    </div>
  </section>
</template>

<style scoped>
.delphi-settings-page {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background: var(--background);
}

.delphi-settings-content-inner {
  width: min(100%, 820px);
  margin: 0 auto;
  padding: 2rem 1.5rem 3rem;
}

.delphi-settings-page--mac .delphi-settings-content-inner {
  padding-top: 2.5rem;
}
</style>
