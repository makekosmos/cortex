<script setup lang="ts">
import { computed } from "vue";
import GeneralSettingsTab from "@/components/settings/GeneralSettingsTab.vue";

const isMac = navigator.platform.startsWith("Mac");

const settingsPageClasses = computed(() => [
  "delphi-settings-page",
  { "delphi-settings-page--mac": isMac },
]);
</script>

<template>
  <section :class="settingsPageClasses">
    <div class="delphi-settings-content">
      <div class="delphi-settings-content-inner">
        <GeneralSettingsTab />
      </div>
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

.delphi-settings-content {
  min-width: 0;
  min-height: 0;
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
