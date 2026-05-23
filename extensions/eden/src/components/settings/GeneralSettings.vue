<template>
  <div class="settings-tab">
    <h1 class="settings-tab-title">Общие</h1>
    <p class="settings-tab-subtitle">Основные настройки пространства</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Хранилище</h2>
        </div>
        <div class="settings-section-body">
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Источник данных</div>
              <div class="settings-row-desc">{{ vaultPath || "ARK (через Kepler)" }}</div>
            </div>
          </div>
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Редактор</h2>
        </div>
        <div class="settings-section-body">
          <label class="settings-row settings-row--toggle">
            <div class="settings-row-left">
              <div class="settings-row-title">Проверка орфографии</div>
              <div class="settings-row-desc">
                Подчёркивает слова с возможными опечатками встроенным проверщиком браузера.
              </div>
            </div>
            <input
              type="checkbox"
              data-testid="eden-spellcheck-toggle"
              :checked="preferences.state.spellcheckEnabled"
              @change="onToggleSpellcheck"
            />
          </label>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { usePreferences } from "@/composables/usePreferences";

defineProps<{
  vaultPath: string;
}>();

defineEmits<{
  selectVault: [];
}>();

const preferences = usePreferences();

function onToggleSpellcheck(e: Event): void {
  const target = e.target as HTMLInputElement;
  preferences.setSpellcheckEnabled(target.checked);
}
</script>

<style scoped>
.settings-row--toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  cursor: pointer;
}
.settings-row--toggle input[type="checkbox"] {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  accent-color: var(--accent-primary, #2aa7ee);
  cursor: pointer;
}
</style>
