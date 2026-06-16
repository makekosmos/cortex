<template>
  <div class="settings" tabindex="0" @keydown="onKey">
    <ToastHost />

    <div class="settings-shell">
      <SettingsSidebar title="Настройки">
        <div class="settings-sidebar-scroll kosmos-scroll">
          <div class="settings-sidebar-group">
            <SettingsSidebarButton
              v-for="item in navigationItems"
              :key="item.id"
              :icon="item.icon"
              :label="item.label"
              :active="tab === item.id"
              :test-id="item.testId"
              @click="tab = item.id"
            />
          </div>
        </div>
      </SettingsSidebar>

      <div class="settings-content">
        <SettingsContentHeader />
        <SettingsPage :initial-tab="tab" @refresh-data="eden.refreshData()" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, shallowRef } from "vue";
import { FileDown, Keyboard, Settings, Trash2 } from "@lucide/vue";
import {
  SettingsContentHeader,
  SettingsSidebar,
  SettingsSidebarButton,
  ToastHost,
  provideToastHost,
} from "@kosmos/visuals";
import "@kosmos/visuals/components/settings-shell.css";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import { useEdenStore } from "@/store/eden";

type SettingsTab = "general" | "export" | "trash" | "vim";

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
    id: "export" as const,
    icon: FileDown,
    label: "Экспорт",
    testId: "eden-settings-export",
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

// Окно настроек — отдельный вью (не App.vue), поэтому стор Eden тут не
// инициализирован: без этого `eden.noteTypes`/`eden.entries` пусты и секции
// «Отображаемые типы» / «Типы для vault-экспорта» рендерятся пустыми.
onMounted(() => {
  void eden.refreshData();
});

function onKey(event: KeyboardEvent): void {
  if (event.key !== "Escape") return;
  event.preventDefault();
  void window.kepler?.edenSettings?.close?.();
}
</script>

<style scoped>
/* Окно настроек Eden — отдельный вью без full-height цепочки html/#root,
   поэтому фиксируем высоту по вьюпорту (shared `.settings` использует 100%). */
.settings {
  height: 100vh;
}
</style>
