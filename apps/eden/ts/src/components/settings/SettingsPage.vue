<template>
  <div :class="settingsPageClasses">
    <KeplerSidebar
      class-name="settings-sidebar-shell"
      :primary-items="navItems"
      :footer-items="footerItems"
      :default-width="236"
      :min-width="220"
      :max-width="300"
      :hidden-width="80"
      :is-mac="isMac"
      drag-region
      :show-toggle="false"
      :top-item="backItem"
      :initial-config="loadSidebarConfig()"
      @config-change="saveSidebarConfig"
    />

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
      <SpacesSettings
        v-else-if="activeTab === 'spaces'"
        :active-space="activeSpace"
        @select-space="emit('selectSpace', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Sidebar as KeplerSidebar,
  type SidebarConfig,
  type SidebarNavItem,
} from "@kepler/visuals";
import type { SpaceId } from "@/components/sidebar/types";
import {
  BackIcon,
  GlobeIcon,
  LinkIcon,
  SettingsIcon,
  ShapesIcon,
  StorageIcon,
  TrashIcon,
} from "@/components/sidebar/edenSidebarIcons";
import GeneralSettings from "./GeneralSettings.vue";
import TrashSettings from "./TrashSettings.vue";
import StorageSettings from "./StorageSettings.vue";
import ObjectTypesSettings from "./ObjectTypesSettings.vue";
import ConnectedAppsSettings from "./ConnectedAppsSettings.vue";
import SpacesSettings from "./SpacesSettings.vue";
import "./SettingsPage.css";

type SettingsTab = "general" | "trash" | "storage" | "object-types" | "connected-apps" | "spaces";

const STORAGE_KEY = "eden-settings-sidebar-config";

const props = defineProps<{
  settings: CodeToolsSettings | null;
  vaultPath: string;
  noteTypes: NoteType[];
  activeSpace: SpaceId;
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
  selectSpace: [spaceId: SpaceId];
  settingsChange: [patch: Partial<CodeToolsSettings>];
  refreshData: [];
}>();

const activeTab = ref<SettingsTab>("general");
const isMac = navigator.platform.startsWith("Mac");
const settingsPageClasses = computed(() => [
  "settings-page",
  { "settings-page--mac": isMac },
]);

function loadSidebarConfig(): Partial<SidebarConfig> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const config = JSON.parse(raw) as Partial<SidebarConfig>;
      return { ...config, hidden: false };
    }
  } catch {
    // ignore
  }
  return { width: 236, hidden: false };
}

function saveSidebarConfig(config: SidebarConfig) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...config, hidden: false }));
}

const navItems = computed<SidebarNavItem[]>(() => [
  {
    id: "general",
    icon: SettingsIcon,
    label: "Общие",
    active: activeTab.value === "general",
    onClick: () => (activeTab.value = "general"),
    testId: "settings-nav-general",
  },
  {
    id: "trash",
    icon: TrashIcon,
    label: "Корзина",
    active: activeTab.value === "trash",
    onClick: () => (activeTab.value = "trash"),
    testId: "settings-nav-trash",
  },
  {
    id: "storage",
    icon: StorageIcon,
    label: "Хранилище",
    active: activeTab.value === "storage",
    onClick: () => (activeTab.value = "storage"),
    testId: "settings-nav-storage",
  },
  {
    id: "connected-apps",
    icon: LinkIcon,
    label: "Связанные программы",
    active: activeTab.value === "connected-apps",
    onClick: () => (activeTab.value = "connected-apps"),
    testId: "settings-nav-connected-apps",
  },
  {
    id: "object-types",
    icon: ShapesIcon,
    label: "Типы объектов",
    active: activeTab.value === "object-types",
    onClick: () => (activeTab.value = "object-types"),
    testId: "settings-nav-object-types",
  },
  {
    id: "spaces",
    icon: GlobeIcon,
    label: "Пространства",
    active: activeTab.value === "spaces",
    onClick: () => (activeTab.value = "spaces"),
    testId: "settings-nav-spaces",
  },
]);

const backItem = computed<SidebarNavItem>(() => ({
  id: "back",
  icon: BackIcon,
  label: "Назад",
  onClick: () => emit("back"),
  testId: "settings-nav-back",
}));

const footerItems = computed<SidebarNavItem[]>(() => []);
</script>
