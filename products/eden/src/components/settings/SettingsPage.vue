<template>
  <div class="settings-page">
    <div class="settings-content">
      <GeneralSettings v-if="activeTab === 'general'" />
      <TrashSettings v-else-if="activeTab === 'trash'" @refresh-data="emit('refreshData')" />
      <VimSettings v-else-if="activeTab === 'vim'" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { shallowRef, watch } from "vue";
import GeneralSettings from "./GeneralSettings.vue";
import TrashSettings from "./TrashSettings.vue";
import VimSettings from "./VimSettings.vue";
import "./SettingsPage.css";

type SettingsTab = "general" | "trash" | "vim";

const props = defineProps<{
  initialTab?: SettingsTab;
}>();

const emit = defineEmits<{
  refreshData: [];
}>();

const activeTab = shallowRef<SettingsTab>(props.initialTab ?? "general");

watch(
  () => props.initialTab,
  (nextTab) => {
    if (!nextTab) {
      return;
    }

    activeTab.value = nextTab;
  },
);
</script>
