<template>
  <div class="settings-tab">
    <h1 class="settings-tab-title">Общие</h1>
    <p class="settings-tab-subtitle">Основные настройки пространства</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Редактор</h2>
        </div>
        <div class="settings-section-body">
          <SettingsList>
            <SettingsToggleRow
              title="Проверка орфографии"
              description="Подчёркивает слова с возможными опечатками встроенным проверщиком браузера."
              :model-value="preferences.state.spellcheckEnabled"
              data-testid="eden-spellcheck-toggle"
              @update:model-value="preferences.setSpellcheckEnabled"
            />
            <SettingsToggleRow
              title="Markdown-редактор (бета)"
              description="CodeMirror 6 с live preview в стиле Obsidian."
              :model-value="preferences.state.cmEditorEnabled"
              data-testid="eden-cm-editor-toggle"
              @update:model-value="preferences.setCmEditorEnabled"
            />
          </SettingsList>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import { usePreferences } from "@/composables/usePreferences";

const preferences = usePreferences();
</script>
