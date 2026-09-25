<script setup lang="ts">
// DictationTab — основной tab диктации: микрофон, язык, hotkey, режим
// триггера, inject mode, provider + статистика.

import { computed, inject, onMounted, ref } from "vue";
import {
  Button,
  HotkeyCapture,
  SettingsButtonRow,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
  SettingsToggleRow,
} from "@kosmos/visuals";
import { ChevronRight, ListRestart, Trash2 } from "@lucide/vue";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import ModelProviderBadge from "./ModelProviderBadge.vue";
import {
  DICTATION_IDLE_UNLOAD_OPTIONS,
  DICTATION_LANGUAGE_OPTIONS,
  DICTATION_TRIGGER_OPTIONS,
  DictationConfigKey,
  type DictationVoiceModelOption,
} from "../composables/useDictationConfig";
import { useDictationPending } from "../composables/useDictationPending";

// Системный hotkey-capture (begin_hotkey_capture) — adapter-метод: ловит даже
// системные сочетания до WebContents через low-level hook (Windows) / CGEventTap
// (macOS). UI платформо-агностичен: всегда external-capture, платформа
// инкапсулирована per-OS внутри backend-адаптера.
const props = defineProps<{
  intro: IntroDescriptor | null;
}>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("DictationTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationMicError,
  dictationCaptureAccelerator,
  dictationCaptureCancelTick,
  dictationProviderDescription,
  dictationVoiceModelOptions,
  dictationVoiceModelValue,
  dictationMicOptions,
  statsCards,
  loadDictationConfig,
  loadDictationStats,
  loadDictationMicrophones,
  onDictationMicChange,
  onDictationLanguageChange,
  onDictationTriggerModeChange,
  onDictationDuckAudioChange,
  onDictationVoiceModelChange,
  onDictationInjectModeChange,
  onDictationHotkeyCapture,
  onDictationCaptureStart,
  onDictationCaptureEnd,
  onDictationIdleUnloadChange,
} = ctx;

const {
  pendingItems,
  pendingError,
  busyUuid,
  retryPending,
  discardPending,
  retryAllPending,
  discardAllPending,
  formatCreatedAt,
  formatDuration,
} = useDictationPending();

type DictationSettingsView = "overview" | "pending";

const activeSettingsView = ref<DictationSettingsView>("overview");

const pendingIntro = computed<IntroDescriptor>(() => ({
  icon: ListRestart,
  label: "Очередь диктовок",
  description: "Сохранённые локально диктовки, которые ждут повторной отправки.",
  iconGradient: props.intro?.iconGradient,
}));

const activeIntro = computed(() =>
  activeSettingsView.value === "pending" ? pendingIntro.value : props.intro,
);

const pendingQueueLabel = computed(() => {
  if (pendingError.value) return "Есть ошибка";
  if (pendingItems.value.length === 0) return "Пусто";
  return String(pendingItems.value.length);
});

function modelOptionBadgeKind(
  option?: DictationVoiceModelOption | null,
): "groq" | "openai" | "nvidia" | "speech" | "text" {
  if (!option) return "speech";
  if (option.iconProvider) return option.iconProvider;
  const lowerLabel = option.label.toLowerCase();
  if (lowerLabel.includes("gpt") || lowerLabel.includes("whisper") || option.provider === "local") {
    return "openai";
  }
  if (option.provider === "groq") return "groq";
  return "speech";
}

function openPendingPage() {
  activeSettingsView.value = "pending";
}

onMounted(async () => {
  void loadDictationStats();
  void loadDictationMicrophones();
  await loadDictationConfig();
  if (dictationConfig.value.injectMode !== "auto_paste") {
    await onDictationInjectModeChange("auto_paste");
  }
});
</script>

<template>
  <AdvancedPageLayout :intro="activeIntro">
    <template v-if="activeSettingsView === 'overview'">
      <section class="advanced-page__section">
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
      </section>

      <section class="advanced-page__section">
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
      </section>

      <section class="advanced-page__section">
        <div class="stats-header">
          <span class="stats-header__title">Основное</span>
        </div>
        <SettingsList>
          <SettingsDropdownRow
            title="Модель"
            :description="dictationProviderDescription"
            :model-value="dictationVoiceModelValue"
            :options="dictationVoiceModelOptions"
            searchable
            search-placeholder="Поиск модели"
            @update:modelValue="onDictationVoiceModelChange"
          >
            <template #trigger-leading="{ option }">
              <ModelProviderBadge :kind="modelOptionBadgeKind(option)" />
            </template>
            <template #option-leading="{ option }">
              <ModelProviderBadge :kind="modelOptionBadgeKind(option)" />
            </template>
          </SettingsDropdownRow>
          <SettingsDropdownRow
            title="Язык"
            description="Подсказка для Whisper. «Авто» — автоопределение."
            :model-value="dictationConfig.language"
            :options="DICTATION_LANGUAGE_OPTIONS"
            @update:modelValue="onDictationLanguageChange"
          />
          <SettingsDropdownRow
            title="Режим триггера"
            description="Как срабатывает горячая клавиша."
            :model-value="dictationConfig.triggerMode"
            :options="DICTATION_TRIGGER_OPTIONS"
            @update:modelValue="onDictationTriggerModeChange"
          />
          <SettingsToggleRow
            title="Приглушать звук"
            description="Во время записи системная громкость временно опускается до 20%."
            :model-value="dictationConfig.duckAudioDuringRecording"
            @update:modelValue="onDictationDuckAudioChange"
          />
          <SettingsDropdownRow
            v-if="dictationConfig.provider === 'local'"
            title="Выгрузка модели"
            description="Через сколько простоя выгружать локальную модель из памяти."
            :model-value="dictationConfig.localIdleUnloadMs"
            :options="DICTATION_IDLE_UNLOAD_OPTIONS"
            @update:modelValue="onDictationIdleUnloadChange"
          />
          <SettingsRow
            title="Вставка"
            description="После распознавания текст автоматически вставляется через Ctrl+V."
          >
            <template #control>
              <span class="settings-static-value">Auto-paste</span>
            </template>
          </SettingsRow>
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
        </SettingsList>
      </section>

      <section class="advanced-page__section">
        <div class="stats-header">
          <span class="stats-header__title">Дополнительно</span>
        </div>
        <SettingsList>
          <button class="settings-nav-row" type="button" @click="openPendingPage">
            <span class="settings-nav-row__text">
              <strong>Очередь диктовок</strong>
            </span>
            <span class="settings-nav-row__meta">{{ pendingQueueLabel }}</span>
            <ChevronRight :size="16" aria-hidden="true" />
          </button>
        </SettingsList>
      </section>
    </template>

    <template v-else>
      <section class="advanced-page__section">
        <div class="stats-header pending-header">
          <span class="stats-header__title">Очередь диктовок</span>
          <button
            v-if="pendingItems.length > 0"
            class="pending-delete-all"
            type="button"
            :disabled="Boolean(busyUuid)"
            @click="discardAllPending()"
          >
            <Trash2 :size="13" aria-hidden="true" />
            Удалить все
          </button>
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
            :description="
              item.lastError || 'Диктовка сохранена локально и ждёт повторной отправки.'
            "
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
          <SettingsRow
            v-if="!pendingError && pendingItems.length === 0"
            title="Очередь пуста"
            description="Неотправленных диктовок сейчас нет."
          />
        </SettingsList>
      </section>
    </template>
  </AdvancedPageLayout>
</template>

<style scoped>
.settings-nav-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 10px;
  align-items: center;
  width: 100%;
  min-height: 48px;
  padding: 10px 12px;
  border: 0;
  color: inherit;
  background: transparent;
  font: inherit;
  text-align: left;
}

.settings-nav-row:hover {
  background: color-mix(in srgb, currentColor 6%, transparent);
}

.settings-nav-row__text {
  display: grid;
  min-width: 0;
}

.settings-nav-row__text strong {
  overflow: hidden;
  font-size: 13px;
  font-weight: 450;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-nav-row__meta {
  color: color-mix(in srgb, currentColor 56%, transparent);
  font-size: 12px;
  font-weight: 650;
  white-space: nowrap;
}

.settings-static-value {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 650;
  white-space: nowrap;
}

.pending-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.pending-delete-all {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 9px;
  border: 1px solid color-mix(in srgb, oklch(0.65 0.22 25) 28%, transparent);
  border-radius: 6px;
  color: oklch(0.72 0.17 25);
  background: transparent;
  font: inherit;
  font-size: 11px;
  font-weight: 650;
  white-space: nowrap;
}

.pending-delete-all:hover:not(:disabled) {
  background: color-mix(in srgb, oklch(0.65 0.22 25) 12%, transparent);
}

.pending-delete-all:disabled {
  cursor: default;
  opacity: 0.45;
}

.pending-item__actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
</style>
