<script setup lang="ts">
// AboutTab — версия Kepler + кнопка «Проверить обновления».
// Часть decomposition'а SettingsView (Шаг 2): tab extraction.
//
// State приходит через props/events — owner-у удобнее держать update lifecycle
// в одном composable `useKeplerUpdate`, а не дублировать его в каждом tab'е.
// DOM-классы (`.advanced-page`, `.row`, `.row-label`, `.label`, `.hint`,
// `.row-actions`, `.value`, `.btn.ghost`) приходят из non-scoped
// `settings-shared.css`, который импортирует SettingsView.

import { SettingsAdvancedIntro } from "@kosmos/visuals";
import type { Component } from "vue";

interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{
  intro: IntroDescriptor | null;
  version: string;
  checkResultLabel: string;
  updateChecking: boolean;
  /** Только нужен `kind === 'downloading'` дальше — но передаём целиком,
   *  чтобы не плодить мелких узких props. */
  isDownloading: boolean;
}>();

defineEmits<{ checkUpdates: [] }>();
</script>

<template>
  <section class="advanced-page kosmos-scroll">
    <SettingsAdvancedIntro
      v-if="intro"
      :icon="intro.icon"
      :title="intro.label"
      :description="intro.description"
      :image-src="intro.introImage"
      :icon-from="intro.iconGradient?.from"
      :icon-to="intro.iconGradient?.to"
    />

    <div class="advanced-page__body rows">
      <div class="row">
        <div class="row-label">
          <div class="label">Версия Kepler</div>
          <div v-if="checkResultLabel" class="hint">{{ checkResultLabel }}</div>
        </div>
        <div class="row-actions">
          <code class="value">{{ version }}</code>
          <button
            type="button"
            class="btn ghost"
            :disabled="updateChecking || isDownloading"
            @click="$emit('checkUpdates')"
          >
            {{ updateChecking ? "Проверяем…" : "Проверить обновления" }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
