<script setup lang="ts">
// DictationTab — основной tab диктации: микрофон, язык, hotkey, режим
// триггера, inject mode, provider + статистика.

import { computed, inject, onMounted } from "vue";
import {
  Button,
  HotkeyCapture,
  SettingsButtonRow,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
} from "@kosmos/visuals";
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
      </div>
      <SettingsList>
        <SettingsButtonRow
          v-if="pendingItems.length > 1"
          title="Все элементы очереди"
          description="Повторно отправить все сохранённые диктовки."
          button-label="Повторить все"
          variant="ghost"
          @click="retryAllPending()"
        />
        <SettingsRow v-if="pendingError" title="Ошибка очереди" :description="pendingError" />
        <SettingsRow
          v-for="item in pendingItems"
          :key="item.uuid"
          :title="`${formatCreatedAt(item.createdAt)} · ${formatDuration(item.durationSec)} · попыток ${item.attempts}${item.language ? ` · ${item.language}` : ''}`"
          :description="item.lastError || 'Диктовка сохранена локально и ждёт повторной отправки.'"
        >
          <template #control>
            <div class="pending-item__actions">
              <Button
                size="sm"
                variant="ghost"
                :disabled="busyUuid === item.uuid"
                :loading="busyUuid === item.uuid"
                @click="retryPending(item.uuid)"
              >
                Повторить
              </Button>
              <Button
                size="sm"
                variant="danger"
                :disabled="busyUuid === item.uuid"
                @click="discardPending(item.uuid)"
              >
                Удалить
              </Button>
            </div>
          </template>
        </SettingsRow>
      </SettingsList>
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

.pending-item__actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
</style>
