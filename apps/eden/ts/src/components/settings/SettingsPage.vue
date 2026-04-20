<template>
  <div class="settings-page">
    <div class="settings-content">
      <GeneralSettings
        v-if="activeTab === 'general'"
        :settings="settings"
        :vault-path="vaultPath"
        @select-vault="emit('selectVault')"
        @settings-change="emit('settingsChange', $event)"
      />
      <TrashSettings v-else-if="activeTab === 'trash'" @refresh-data="emit('refreshData')" />
      <StorageSettings v-else-if="activeTab === 'storage'" :vault-path="vaultPath" />
      <ConnectedAppsSettings
        v-else-if="activeTab === 'connected-apps'"
        @refresh-data="emit('refreshData')"
      />
      <SpacesSettings
        v-else-if="activeTab === 'spaces'"
        :active-space="activeSpace"
        @select-space="emit('selectSpace', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { shallowRef, watch } from "vue";
import type { SpaceId } from "@/components/sidebar/types";
import GeneralSettings from "./GeneralSettings.vue";
import TrashSettings from "./TrashSettings.vue";
import StorageSettings from "./StorageSettings.vue";
import ConnectedAppsSettings from "./ConnectedAppsSettings.vue";
import SpacesSettings from "./SpacesSettings.vue";
import "./SettingsPage.css";

type SettingsTab = "general" | "trash" | "storage" | "connected-apps" | "spaces";

const props = defineProps<{
  settings: CodeToolsSettings | null;
  vaultPath: string;
  activeSpace: SpaceId;
  initialTab?: SettingsTab;
}>();

const emit = defineEmits<{
  selectVault: [];
  selectSpace: [spaceId: SpaceId];
  settingsChange: [patch: Partial<CodeToolsSettings>];
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
