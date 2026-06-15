<template>
  <div class="eden-settings" tabindex="0" @keydown="onKey">
    <ToastHost />

    <SettingsSidebar title="Eden" tone="strong">
      <div class="eden-settings-sidebar-header">
        <div class="eden-settings-sidebar-title">Настройки</div>
        <div class="eden-settings-sidebar-subtitle">Параметры заметок и хранения</div>
      </div>

      <div class="eden-settings-sidebar-scroll kosmos-scroll">
        <SettingsSidebarButton
          v-for="item in navigationItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label"
          :active="tab === item.id"
          :test-id="item.testId"
          icon-variant="plain"
          @click="tab = item.id"
        />
      </div>
    </SettingsSidebar>

    <div class="eden-settings-content">
      <SettingsPage :initial-tab="tab" @refresh-data="eden.refreshData()" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { shallowRef } from "vue";
import { Keyboard, Settings, Trash2 } from "@lucide/vue";
import {
  SettingsSidebar,
  SettingsSidebarButton,
  ToastHost,
  provideToastHost,
} from "@kosmos/visuals";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import { useEdenStore } from "@/store/eden";

type SettingsTab = "general" | "trash" | "vim";

const eden = useEdenStore();
const tab = shallowRef<SettingsTab>("general");

const navigationItems = [
  {
    id: "general" as const,
    icon: Settings,
    label: "Общие",
    testId: "eden-settings-general",
  },
  {
    id: "trash" as const,
    icon: Trash2,
    label: "Корзина",
    testId: "eden-settings-trash",
  },
  {
    id: "vim" as const,
    icon: Keyboard,
    label: "Vim",
    testId: "eden-settings-vim",
  },
];

provideToastHost();

function onKey(event: KeyboardEvent): void {
  if (event.key !== "Escape") return;
  event.preventDefault();
  void window.kepler?.edenSettings?.close?.();
}
</script>

<style scoped>
.eden-settings {
  display: flex;
  min-width: 0;
  min-height: 0;
  width: 100vw;
  height: 100vh;
  background: var(--color-bg-primary);
}

.eden-settings-sidebar-header {
  display: grid;
  gap: 2px;
  padding: 12px 12px 8px;
}

.eden-settings-sidebar-title {
  color: var(--text-primary);
  font-size: 14px;
  font-weight: 700;
  line-height: 1.3;
}

.eden-settings-sidebar-subtitle {
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.4;
}

.eden-settings-sidebar-scroll {
  display: flex;
  min-height: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 4px;
  padding: 0 8px 12px;
  overflow-y: auto;
}

.eden-settings-content {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  overflow: hidden;
}
</style>
