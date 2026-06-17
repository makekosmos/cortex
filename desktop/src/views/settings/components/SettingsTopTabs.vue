<script setup lang="ts">
// macOS-стиль навигации настроек: горизонтальный таб-бар сверху (как Raycast),
// иконка над лейблом, активный таб — rounded highlight. Используется вместо
// вертикального SettingsSidebar только на macOS (см. SettingsView).
//
// Переиспользует те же SettingsNavigationItem, что и sidebar — никакой
// дубль-навигации, только другой layout.

import type { SettingsNavigationItem, Tab } from "../navigation";

defineProps<{
  items: SettingsNavigationItem[];
  activeTab: Tab | null;
}>();

defineEmits<{ select: [tab: Tab] }>();
</script>

<template>
  <nav class="mac-tabs">
    <button
      v-for="item in items"
      :key="item.tab"
      type="button"
      class="mac-tab"
      :class="{ 'mac-tab--active': item.tab === activeTab }"
      @click="$emit('select', item.tab)"
    >
      <span class="mac-tab__icon">
        <component :is="item.icon" :size="20" :stroke-width="1.75" />
      </span>
      <span class="mac-tab__label">{{ item.label }}</span>
    </button>
  </nav>
</template>

<style scoped>
.mac-tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: center;
  gap: 2px;
  /* Симметричный горизонтальный отступ ≥ ширины traffic lights: центрируем
     табы в зоне правее нативных контролов, чтобы левый таб не налезал на
     traffic lights. Сверху — чуть-чуть, чтобы первый ряд опустился ниже
     зоны контролов. */
  padding: 8px max(12px, var(--kosmos-mac-traffic-light-left-safe-area, 0px)) 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 10%, transparent);
  -webkit-app-region: drag;
}

.mac-tab {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  min-width: 64px;
  padding: 8px 10px 7px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 62%, transparent);
  cursor: default;
  transition:
    background-color 140ms var(--easing-standard, ease),
    color 140ms var(--easing-standard, ease);
  /* Сами табы кликабельны — выключаем drag на них. */
  -webkit-app-region: no-drag;
}

.mac-tab:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
}

.mac-tab--active,
.mac-tab--active:hover {
  background: color-mix(in srgb, var(--foreground) 12%, transparent);
  color: var(--foreground);
}

.mac-tab__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 22px;
}

.mac-tab__label {
  font-size: var(--kosmos-text-caption-size, 0.6875rem);
  line-height: 1;
  font-weight: 500;
  white-space: nowrap;
}
</style>
