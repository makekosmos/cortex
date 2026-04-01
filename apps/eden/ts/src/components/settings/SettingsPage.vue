<template>
  <div class="settings-page">
    <nav class="settings-nav">
      <div class="settings-nav-head">
        <button class="settings-nav-back" type="button" @click="emit('back')">
          <svg
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          >
            <path d="M12 4L6 10L12 16" />
          </svg>
        </button>
        <span class="settings-nav-head-title">Настройки</span>
      </div>
      <div v-for="section in navSections" :key="section.title" class="settings-nav-group">
        <div class="settings-nav-section">{{ section.title }}</div>
        <button
          v-for="item in section.items"
          :key="item.id"
          class="settings-nav-item"
          :class="{ active: activeTab === item.id }"
          type="button"
          @click="activeTab = item.id"
        >
          <span class="settings-nav-icon icon" :class="item.icon" />
          <span class="settings-nav-label">{{ item.label }}</span>
        </button>
      </div>
    </nav>
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
      <ObjectTypesSettings
        v-else-if="activeTab === 'object-types'"
        :note-types="noteTypes"
        :on-note-type-save="onNoteTypeSave"
        :on-note-type-delete="onNoteTypeDelete"
      />
      <ConnectedAppsSettings
        v-else-if="activeTab === 'connected-apps'"
        @refresh-data="emit('refreshData')"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import GeneralSettings from "./GeneralSettings.vue";
import TrashSettings from "./TrashSettings.vue";
import StorageSettings from "./StorageSettings.vue";
import ObjectTypesSettings from "./ObjectTypesSettings.vue";
import ConnectedAppsSettings from "./ConnectedAppsSettings.vue";
import "./SettingsPage.css";

type SettingsTab = "general" | "trash" | "storage" | "object-types" | "connected-apps";

defineProps<{
  settings: CodeToolsSettings | null;
  vaultPath: string;
  noteTypes: NoteType[];
  onNoteTypeSave: (
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;
      slug?: string;
    },
  ) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
}>();

const emit = defineEmits<{
  back: [];
  selectVault: [];
  settingsChange: [patch: Partial<CodeToolsSettings>];
  refreshData: [];
}>();

const activeTab = ref<SettingsTab>("general");

const navSections = [
  {
    title: "Настройки",
    items: [
      { id: "general" as SettingsTab, label: "Общие", icon: "settings-space" },
      { id: "trash" as SettingsTab, label: "Корзина", icon: "settings-bin" },
      { id: "storage" as SettingsTab, label: "Хранилище", icon: "settings-storage" },
      { id: "connected-apps" as SettingsTab, label: "Связанные программы", icon: "settings-space" },
    ],
  },
  {
    title: "Модель содержимого",
    items: [{ id: "object-types" as SettingsTab, label: "Типы объектов", icon: "settings-type" }],
  },
];
</script>
