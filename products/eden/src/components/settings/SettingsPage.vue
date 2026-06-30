<template>
  <GeneralSettings v-if="props.activeTab === 'general'" />
  <ExportSettings v-else-if="props.activeTab === 'export'" />
  <TrashSettings v-else-if="props.activeTab === 'trash'" @refresh-data="emit('refreshData')" />
</template>

<script setup lang="ts">
import GeneralSettings from "./GeneralSettings.vue";
import ExportSettings from "./ExportSettings.vue";
import TrashSettings from "./TrashSettings.vue";
import "./SettingsPage.css";

type SettingsTab = "general" | "export" | "trash";

const props = defineProps<{
  activeTab: SettingsTab;
}>();

const emit = defineEmits<{
  refreshData: [];
}>();
</script>
