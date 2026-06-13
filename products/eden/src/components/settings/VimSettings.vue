<template>
  <div class="settings-tab vim-settings">
    <h1 class="settings-tab-title">Vim</h1>
    <p class="settings-tab-subtitle">Режим команд для Markdown-редактора Eden</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Режим</h2>
        </div>
        <div class="settings-section-body">
          <SettingsList>
            <SettingsToggleRow
              title="Vim-режим"
              description="Включает Vim-команды в CodeMirror-редакторе. Сами сочетания фиксированы."
              :model-value="preferences.state.vimModeEnabled"
              data-testid="eden-vim-mode-toggle"
              @update:model-value="preferences.setVimModeEnabled"
            />
          </SettingsList>
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Действия</h2>
        </div>
        <div class="settings-section-body vim-motion-body">
          <div
            v-for="group in VIM_MOTION_GROUPS"
            :key="group.id"
            class="vim-motion-group"
            :data-testid="`vim-motion-group-${group.id}`"
          >
            <h3 class="vim-motion-group-title">{{ group.title }}</h3>
            <div class="vim-motion-list">
              <div v-for="item in group.items" :key="item.keys" class="vim-motion-row">
                <kbd class="vim-motion-key">{{ item.keys }}</kbd>
                <div class="vim-motion-copy">
                  <div class="vim-motion-title">{{ item.title }}</div>
                  <div class="vim-motion-description">{{ item.description }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup vapor lang="ts">
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import { usePreferences } from "@/composables/usePreferences";
import { VIM_MOTION_GROUPS } from "@/editor-cm/vimMotions";

const preferences = usePreferences();
</script>

<style scoped>
.vim-motion-body {
  gap: 18px;
  padding: 16px;
}

.vim-motion-group {
  display: grid;
  gap: 8px;
}

.vim-motion-group-title {
  margin: 0;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.vim-motion-list {
  display: grid;
  gap: 1px;
  overflow: hidden;
  border: 1px solid var(--color-shape-secondary);
  border-radius: 8px;
  background: var(--color-shape-secondary);
}

.vim-motion-row {
  display: grid;
  grid-template-columns: minmax(92px, 130px) minmax(0, 1fr);
  gap: 14px;
  align-items: start;
  padding: 10px 12px;
  background: var(--color-shape-highlight-light-solid);
}

.vim-motion-key {
  justify-self: start;
  max-width: 100%;
  padding: 3px 7px;
  border: 1px solid var(--color-shape-secondary);
  border-radius: 6px;
  background: var(--color-bg-primary);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.4;
  white-space: normal;
}

.vim-motion-copy {
  min-width: 0;
  display: grid;
  gap: 3px;
}

.vim-motion-title {
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
}

.vim-motion-description {
  color: var(--text-tertiary);
  font-size: 12px;
  line-height: 1.45;
}

@media (max-width: 640px) {
  .vim-motion-row {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
