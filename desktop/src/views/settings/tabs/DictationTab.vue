<script setup lang="ts">
// DictationTab — основной tab диктации: микрофон, язык, hotkey, режим
// триггера, inject mode, provider + статистика.

import { computed, inject, onMounted } from "vue";
import { HotkeyCapture, SettingsDropdownRow, SettingsList, SettingsRow } from "@kosmos/visuals";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import AppCommandsTab from "./AppCommandsTab.vue";
import {
  DICTATION_INJECT_OPTIONS,
  DICTATION_LANGUAGE_OPTIONS,
  DICTATION_PROVIDER_OPTIONS,
  DICTATION_TRIGGER_OPTIONS,
  DictationConfigKey,
} from "../composables/useDictationConfig";
import { useDictationPending } from "../composables/useDictationPending";
import type { AppCommandSetting } from "../navigation";

const emit = defineEmits<{
  toggleUsageTracker: [e: Event];
  toggleCommandVisibility: [id: string, e: Event];
}>();

// Системный hotkey-capture (begin_hotkey_capture) — adapter-метод: ловит даже
// системные сочетания до WebContents через low-level hook (Windows) / CGEventTap
// (macOS). UI платформо-агностичен: всегда external-capture, платформа
// инкапсулирована per-OS внутри backend-адаптера.
const props = defineProps<{
  intro: IntroDescriptor | null;
  commands: AppCommandSetting[];
  usageTracker: boolean;
  isCommandVisible: (id: string) => boolean;
}>();

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

const commandsWithBinding = computed<AppCommandSetting[]>(() =>
  props.commands.map((command) =>
    command.id === "kepler:dictation"
      ? { ...command, shortcut: dictationConfig.value.hotkey }
      : command,
  ),
);

const {
  pendingItems,
  pendingError,
  busyUuid,
  retryPending,
  discardPending,
  retryAllPending,
  formatCreatedAt,
  formatDuration,
} = useDictationPending();

onMounted(() => {
  void loadDictationConfig();
  void loadDictationStats();
  void loadDictationMicrophones();
});
</script>

<template>
  <AdvancedPageLayout :intro="intro">
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

    <template v-if="pendingItems.length > 0 || pendingError">
      <div class="stats-header pending-header">
        <span class="stats-header__title">Очередь диктовок</span>
        <button
          v-if="pendingItems.length > 1"
          type="button"
          class="pending-retry-all"
          @click="retryAllPending()"
        >
          Повторить все
        </button>
      </div>
      <div v-if="pendingError" class="pending-error">{{ pendingError }}</div>
      <div v-if="pendingItems.length > 0" class="pending-list">
        <div v-for="item in pendingItems" :key="item.uuid" class="pending-item">
          <div class="pending-item__main">
            <div class="pending-item__meta">
              {{ formatCreatedAt(item.createdAt) }} · {{ formatDuration(item.durationSec) }} ·
              попыток {{ item.attempts }}
              <span v-if="item.language"> · {{ item.language }}</span>
            </div>
            <div v-if="item.lastError" class="pending-item__error">
              {{ item.lastError }}
            </div>
          </div>
          <div class="pending-item__actions">
            <button
              type="button"
              class="pending-btn pending-btn--primary"
              :disabled="busyUuid === item.uuid"
              @click="retryPending(item.uuid)"
            >
              {{ busyUuid === item.uuid ? "..." : "Повторить" }}
            </button>
            <button
              type="button"
              class="pending-btn"
              :disabled="busyUuid === item.uuid"
              @click="discardPending(item.uuid)"
            >
              Удалить
            </button>
          </div>
        </div>
      </div>
    </template>

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
    <AppCommandsTab
      :intro="null"
      active-tab="dictation"
      :commands="commandsWithBinding"
      :usage-tracker="usageTracker"
      :is-command-visible="isCommandVisible"
      @toggle-usage-tracker="(e: Event) => emit('toggleUsageTracker', e)"
      @toggle-command-visibility="(id: string, e: Event) => emit('toggleCommandVisibility', id, e)"
    />
  </AdvancedPageLayout>
</template>

<style scoped>
.pending-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.pending-retry-all {
  background: transparent;
  border: 1px solid color-mix(in srgb, var(--foreground) 15%, transparent);
  color: var(--foreground);
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 0.6875rem;
}

.pending-retry-all:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.pending-error {
  font-size: 0.75rem;
  color: #f5a524;
  padding: 8px 12px;
  margin-bottom: 8px;
}

.pending-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 16px;
}

.pending-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.pending-item__main {
  flex: 1;
  min-width: 0;
}

.pending-item__meta {
  font-size: 0.75rem;
  color: var(--foreground);
}

.pending-item__error {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pending-item__actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.pending-btn {
  background: transparent;
  border: 1px solid color-mix(in srgb, var(--foreground) 15%, transparent);
  color: var(--foreground);
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 0.6875rem;
}

.pending-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.pending-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.pending-btn--primary {
  border-color: color-mix(in srgb, var(--foreground) 35%, transparent);
}
</style>
