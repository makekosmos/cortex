<script setup lang="ts">
import { SettingsButtonRow, SettingsList } from "@kosmos/visuals";
import { useTheme } from "@/composables/useTheme";

const { theme, setTheme } = useTheme();
</script>

<template>
  <div class="settings-tab" data-testid="delphi-settings-general-tab">
    <header class="settings-tab-header">
      <p class="settings-tab-kicker">Настройки Delphi</p>
      <h1 class="settings-tab-title">Общие</h1>
      <p class="settings-tab-subtitle">Базовые настройки интерфейса и поведения приложения.</p>
    </header>

    <SettingsList>
      <SettingsButtonRow
        title="Светлая тема"
        description="Яркая тема для дневной работы."
        :button-label="theme === 'light' ? 'Выбрана' : 'Выбрать'"
        :variant="theme === 'light' ? 'primary' : 'ghost'"
        @click="setTheme('light')"
      />
      <SettingsButtonRow
        title="Тёмная тема"
        description="Более контрастный интерфейс для вечера."
        :button-label="theme === 'dark' ? 'Выбрана' : 'Выбрать'"
        :variant="theme === 'dark' ? 'primary' : 'ghost'"
        @click="setTheme('dark')"
      />
      <SettingsButtonRow
        title="Системная тема"
        description="Следовать системной теме устройства."
        :button-label="theme === 'system' ? 'Выбрана' : 'Выбрать'"
        :variant="theme === 'system' ? 'primary' : 'ghost'"
        @click="setTheme('system')"
      />
    </SettingsList>
  </div>
</template>

<style scoped>
.settings-tab {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.settings-tab-header {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.settings-tab-kicker {
  font-size: var(--kosmos-text-caption-size);
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.settings-tab-title {
  font-size: var(--kosmos-text-page-title-size);
  line-height: var(--kosmos-text-page-title-line-height);
  font-weight: var(--kosmos-text-page-title-weight);
  color: var(--foreground);
}

.settings-tab-subtitle {
  max-width: 42rem;
  font-size: var(--kosmos-text-body-size);
  color: var(--muted-foreground);
}
</style>
