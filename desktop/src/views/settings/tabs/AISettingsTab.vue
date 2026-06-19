<script setup lang="ts">
// AISettingsTab — advanced settings для Groq / local transcription setup.

import { computed, inject, onMounted } from "vue";
import {
  Button,
  SettingsButtonRow,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
  SettingsTextInputRow,
  SettingsToggleRow,
} from "@kosmos/visuals";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import {
  DICTATION_LOCAL_ENGINE_OPTIONS,
  DictationConfigKey,
} from "../composables/useDictationConfig";

const props = defineProps<{
  intro: IntroDescriptor | null;
}>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("AISettingsTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationHasApiKey,
  dictationGroqStatus,
  dictationLocalStatus,
  dictationModelName,
  dictationLocalModelPath,
  dictationLocalCommandPath,
  dictationLocalModelId,
  dictationLocalEngine,
  dictationLocalModels,
  dictationLocalModelsBusy,
  dictationLocalModelsError,
  dictationLocalModelDownloadProgress,
  dictationConnTestBusy,
  dictationConnTestResult,
  loadDictationConfig,
  onDictationProviderEnabledChange,
  onDictationModelBlur,
  onDictationLocalModelPathBlur,
  onDictationLocalCommandPathBlur,
  onDictationLocalModelIdBlur,
  onDictationLocalEngineChange,
  onDictationDownloadLocalModel,
  onDictationUseLocalModel,
  onDictationDeleteLocalModel,
  onDictationTestConnectivity,
} = ctx;

const providerLabel = computed(() => {
  switch (dictationConfig.value.provider) {
    case "groq":
      return "Groq";
    case "local":
      return "Локально";
    case "mock":
      return "Тестовый режим";
    default:
      return dictationConfig.value.provider;
  }
});

const isGroqProvider = computed(() => dictationConfig.value.provider === "groq");
const isLocalProvider = computed(() => dictationConfig.value.provider === "local");

const groqConnectionLabel = computed(() =>
  dictationConnTestBusy.value ? "Проверяю…" : "Проверить соединение",
);

const groqConnectionDescription = computed(
  () =>
    dictationConnTestResult.value ||
    "Проверяет доступность Groq через текущие DNS/Proxy настройки.",
);

function localModelDescription(model: {
  id: string;
  description: string;
  sizeMb: number;
  speedScore: number;
  accuracyScore: number;
  downloaded: boolean;
  selected: boolean;
}) {
  const state = model.selected ? "используется" : model.downloaded ? "скачана" : "не скачана";
  const progress = dictationLocalModelDownloadProgress.value[model.id];
  const progressText = progress ? ` · ${downloadProgressLabel(progress)}` : "";
  return `${model.description} · ${model.sizeMb} MB · скорость ${Math.round(
    model.speedScore * 100,
  )}% · качество ${Math.round(model.accuracyScore * 100)}% · ${state}${progressText}`;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 100 || unit === 0 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

function downloadProgressLabel(progress: {
  phase: string;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
}) {
  const phase =
    progress.phase === "tool"
      ? "скачиваю whisper.cpp"
      : progress.phase === "extract"
        ? "распаковываю whisper.cpp"
        : "скачиваю модель";
  if (progress.percent !== null) return `${phase} ${Math.round(progress.percent)}%`;
  if (progress.totalBytes) {
    return `${phase} ${formatBytes(progress.downloadedBytes)} / ${formatBytes(progress.totalBytes)}`;
  }
  return `${phase} ${formatBytes(progress.downloadedBytes)}`;
}

function localModelButtonLabel(model: { id: string; downloaded: boolean; selected: boolean }) {
  const progress = dictationLocalModelDownloadProgress.value[model.id];
  if (progress) return progress.percent !== null ? `${Math.round(progress.percent)}%` : "Скачиваю…";
  if (model.selected) return "Используется";
  if (model.downloaded) return "Использовать";
  return "Скачать и использовать";
}

function hasAnyLocalModelDownload() {
  return Object.keys(dictationLocalModelDownloadProgress.value).length > 0;
}

function localModelProgressPercent(modelId: string) {
  const progress = dictationLocalModelDownloadProgress.value[modelId];
  return Math.max(0, Math.min(100, progress?.percent ?? 0));
}

function onLocalModelAction(model: { id: string; downloaded: boolean; selected: boolean }) {
  if (model.selected || hasAnyLocalModelDownload()) return;
  if (model.downloaded) {
    void onDictationUseLocalModel(model.id);
  } else {
    void onDictationDownloadLocalModel(model.id);
  }
}

function onLocalModelDelete(model: { id: string; downloaded: boolean }) {
  if (!model.downloaded) return;
  void onDictationDeleteLocalModel(model.id);
}

onMounted(() => {
  void loadDictationConfig();
});
</script>

<template>
  <AdvancedPageLayout :intro="intro">
    <SettingsList>
      <SettingsRow
        title="Текущий провайдер"
        description="Переключение делается во вкладке «Диктация»."
      >
        <template #control>
          <span>{{ providerLabel }}</span>
        </template>
      </SettingsRow>
    </SettingsList>

    <h2 class="advanced-section-title">Groq</h2>
    <SettingsList>
      <SettingsToggleRow
        title="Включить Groq"
        description="Ключ хранится в Windows Credential Manager."
        :model-value="dictationConfig.providerEnabled"
        :muted="!isGroqProvider"
        :disabled="!dictationHasApiKey"
        @update:modelValue="onDictationProviderEnabledChange"
      />
      <SettingsTextInputRow
        v-model="dictationModelName"
        title="Модель"
        description="Основная модель для онлайн-транскрипции."
        placeholder="whisper-large-v3"
        :muted="!isGroqProvider"
        @blur="onDictationModelBlur"
      />
      <SettingsRow title="Статус Groq" :description="dictationGroqStatus" :muted="!isGroqProvider">
        <template #control>
          <span>{{ dictationHasApiKey ? "Ключ есть" : "Ключа нет" }}</span>
        </template>
      </SettingsRow>
      <SettingsButtonRow
        title="Проверка соединения"
        :description="groqConnectionDescription"
        :muted="!isGroqProvider"
        variant="ghost"
        :loading="dictationConnTestBusy"
        :disabled="dictationConnTestBusy"
        :button-label="groqConnectionLabel"
        @click="onDictationTestConnectivity"
      />
    </SettingsList>

    <h2 class="advanced-section-title">Локально</h2>
    <SettingsList>
      <SettingsRow
        title="Папка моделей"
        :description="
          dictationLocalModels?.modelsDir || 'Модели будут храниться в папке данных Kosmos.'
        "
      >
        <template #control>
          <span>{{
            dictationLocalModels?.commandInstalled
              ? "whisper.cpp установлен"
              : "whisper.cpp будет скачан"
          }}</span>
        </template>
      </SettingsRow>
      <SettingsRow
        v-if="dictationLocalModelsError"
        title="Ошибка локальных моделей"
        :description="dictationLocalModelsError"
      />
      <SettingsRow
        v-for="model in dictationLocalModels?.models ?? []"
        :key="model.id"
        :title="model.name"
        :description="localModelDescription(model)"
      >
        <template #control>
          <div class="local-model-actions">
            <Button
              size="sm"
              :variant="model.selected ? 'ghost' : model.downloaded ? 'primary' : 'ghost'"
              :loading="
                dictationLocalModelsBusy === model.id ||
                Boolean(dictationLocalModelDownloadProgress[model.id])
              "
              :disabled="
                Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload() || model.selected
              "
              @click="onLocalModelAction(model)"
            >
              {{ localModelButtonLabel(model) }}
            </Button>
            <Button
              v-if="model.downloaded"
              size="sm"
              variant="danger"
              :disabled="Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload()"
              @click="onLocalModelDelete(model)"
            >
              Удалить
            </Button>
            <div
              v-if="dictationLocalModelDownloadProgress[model.id]"
              class="local-model-progress"
              role="progressbar"
              :aria-valuemin="0"
              :aria-valuemax="100"
              :aria-valuenow="localModelProgressPercent(model.id)"
              :aria-label="downloadProgressLabel(dictationLocalModelDownloadProgress[model.id])"
            >
              <div
                class="local-model-progress-fill"
                :style="{ width: `${localModelProgressPercent(model.id)}%` }"
              />
            </div>
          </div>
        </template>
      </SettingsRow>
    </SettingsList>

    <h2 class="advanced-section-title">Ручная настройка</h2>
    <SettingsList>
      <SettingsTextInputRow
        v-model="dictationLocalModelPath"
        title="Путь к модели"
        description="Файл локальной Whisper-модели, например ggml-large-v3-turbo.bin."
        placeholder="D:\Models\ggml-large-v3-turbo.bin"
        :muted="!isLocalProvider"
        @blur="onDictationLocalModelPathBlur"
      />
      <SettingsTextInputRow
        v-model="dictationLocalCommandPath"
        title="Путь к whisper.cpp"
        description="Executable локального распознавания: whisper-cli.exe или main.exe."
        placeholder="D:\Tools\whisper.cpp\whisper-cli.exe"
        :muted="!isLocalProvider"
        @blur="onDictationLocalCommandPathBlur"
      />
      <SettingsTextInputRow
        v-model="dictationLocalModelId"
        title="ID модели"
        description="Идентификатор модели для локального движка."
        placeholder="whisper-large-v3"
        :muted="!isLocalProvider"
        @blur="onDictationLocalModelIdBlur"
      />
      <SettingsDropdownRow
        title="Движок"
        description="Как запускать локальную модель."
        :model-value="dictationLocalEngine"
        :options="DICTATION_LOCAL_ENGINE_OPTIONS"
        :muted="!isLocalProvider"
        @update:modelValue="onDictationLocalEngineChange"
      />
      <SettingsRow
        title="Статус локально"
        :description="dictationLocalStatus"
        :muted="!isLocalProvider"
      >
        <template #control>
          <span>{{ isLocalProvider ? "Активно" : "Подготовка" }}</span>
        </template>
      </SettingsRow>
    </SettingsList>
  </AdvancedPageLayout>
</template>

<style scoped>
.local-model-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  min-width: 220px;
}

.local-model-progress {
  width: 100%;
  height: 4px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 18%, transparent);
}

.local-model-progress-fill {
  height: 100%;
  min-width: 4px;
  border-radius: inherit;
  background: currentColor;
  transition: width 160ms ease;
}
</style>
