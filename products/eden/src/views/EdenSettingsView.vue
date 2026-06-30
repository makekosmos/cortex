<template>
  <div class="settings" tabindex="0" @keydown="onKey">
    <ToastHost />

    <div class="settings-shell">
      <SettingsSidebar title="Настройки">
        <div class="settings-sidebar-search">
          <SettingsSearchInput v-model="searchQuery" placeholder="Поиск" />
        </div>

        <div class="settings-sidebar-scroll kosmos-scroll">
          <div v-if="hasSidebarMatches" class="settings-sidebar-group">
            <SettingsSidebarButton
              v-for="item in matchedNavigationItems"
              :key="item.id"
              :icon="item.icon"
              :label="item.label"
              :active="activeTab === item.id"
              :test-id="item.testId"
              @click="selectTab(item.id)"
            />
          </div>

          <div v-else class="settings-sidebar-empty">Ничего не найдено</div>
        </div>
      </SettingsSidebar>

      <div class="settings-content">
        <SettingsContentHeader />
        <div v-if="searchQuery && !activeTab" class="empty">Ничего не найдено</div>
        <SettingsPage
          v-else-if="activeTab"
          :active-tab="activeTab"
          @refresh-data="eden.refreshData()"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import { FileDown, Keyboard, Settings, Trash2 } from "@lucide/vue";
import {
  SettingsContentHeader,
  SettingsSearchInput,
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
const searchQuery = ref("");

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

function normalizeSearchValue(value: string): string {
  return value.trim().toLowerCase();
}

const matchedNavigationItems = computed(() => {
  const query = normalizeSearchValue(searchQuery.value);
  if (!query) return navigationItems;
  return navigationItems.filter((item) => item.label.toLowerCase().includes(query));
});

const hasSidebarMatches = computed(() => matchedNavigationItems.value.length > 0);

const activeTab = computed<SettingsTab | null>(() => {
  if (!searchQuery.value.trim()) return tab.value;
  if (matchedNavigationItems.value.some((item) => item.id === tab.value)) return tab.value;
  return matchedNavigationItems.value[0]?.id ?? null;
});

function selectTab(nextTab: SettingsTab): void {
  tab.value = nextTab;
}

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
