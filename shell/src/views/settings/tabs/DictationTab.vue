<script setup lang="ts">
// DictationTab — основной tab диктации: микрофон, язык, hotkey, режим
// триггера, inject mode, provider + статистика.

import { inject, onMounted, type Component } from "vue";
import {
  HotkeyCapture,
  SettingsAdvancedIntro,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
} from "@kosmos/visuals";
import {
  DICTATION_INJECT_OPTIONS,
  DICTATION_LANGUAGE_OPTIONS,
  DICTATION_PROVIDER_OPTIONS,
  DICTATION_TRIGGER_OPTIONS,
  DictationConfigKey,
} from "../composables/useDictationConfig";

interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{ intro: IntroDescriptor | null }>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("DictationTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationMicError,
  dictationCaptureAccelerator,
  dictationCaptureCancelTick,
  dictationProviderDescription,
  dictationMicOptions,
  statsCards,
  loadDictationConfig,
  loadDictationStats,
  loadDictationMicrophones,
  onDictationMicChange,
  onDictationLanguageChange,
  onDictationInjectModeChange,
  onDictationTriggerModeChange,
  onDictationProviderChange,
  onDictationHotkeyCapture,
  onDictationCaptureStart,
  onDictationCaptureEnd,
} = ctx;

onMounted(() => {
  void loadDictationConfig();
  void loadDictationStats();
  void loadDictationMicrophones();
});
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
    <div class="advanced-page__body">
      <div class="stats-header">
        <span class="stats-header__title">Статистика</span>
      </div>
      <div class="stats-cards">
        <div v-for="card in statsCards" :key="card.key" class="stats-card">
          <div class="stats-card__label">{{ card.label }}</div>
          <div class="stats-card__value">
            <span class="stats-card__number">{{ card.value }}</span>
            <span v-if="card.unit" class="stats-card__unit">{{ card.unit }}</span>
          </div>
        </div>
      </div>

      <div class="stats-header">
        <span class="stats-header__title">Микрофон</span>
      </div>
      <SettingsList>
        <SettingsDropdownRow
          title="Устройство"
          :description="
            dictationMicError ||
            'Если устройство не доступно во время записи — будет fallback на системный default.'
          "
          :model-value="dictationConfig.microphoneDeviceId ?? 'default'"
          :options="dictationMicOptions"
          @update:modelValue="onDictationMicChange"
        />
      </SettingsList>

      <div class="stats-header">
        <span class="stats-header__title">Основное</span>
      </div>
      <SettingsList>
        <SettingsDropdownRow
          title="Язык"
          description="Подсказка для Whisper. «Авто» — автоопределение."
          :model-value="dictationConfig.language"
          :options="DICTATION_LANGUAGE_OPTIONS"
          @update:modelValue="onDictationLanguageChange"
        />
        <SettingsRow title="Горячая клавиша">
          <template #control>
            <HotkeyCapture
              :model-value="dictationConfig.hotkey"
              capture-prompt="Нажми сочетание…"
              external-capture
              :pending-accelerator="dictationCaptureAccelerator"
              :pending-cancel="dictationCaptureCancelTick"
              @capture-start="onDictationCaptureStart"
              @capture-end="onDictationCaptureEnd"
              @update:modelValue="onDictationHotkeyCapture"
            />
          </template>
        </SettingsRow>
        <SettingsDropdownRow
          title="Режим триггера"
          description="Как срабатывает горячая клавиша."
          :model-value="dictationConfig.triggerMode"
          :options="DICTATION_TRIGGER_OPTIONS"
          @update:modelValue="onDictationTriggerModeChange"
        />
        <SettingsDropdownRow
          title="Вставка"
          description="Auto-paste симулирует Ctrl+V и восстанавливает буфер. Clipboard — только записать текст, вы жмёте Ctrl+V сами."
          :model-value="dictationConfig.injectMode"
          :options="DICTATION_INJECT_OPTIONS"
          @update:modelValue="onDictationInjectModeChange"
        />
        <SettingsDropdownRow
          title="Поставщик"
          :description="dictationProviderDescription"
          :model-value="dictationConfig.provider"
          :options="DICTATION_PROVIDER_OPTIONS"
          @update:modelValue="onDictationProviderChange"
        />
      </SettingsList>
    </div>
  </section>
</template>
