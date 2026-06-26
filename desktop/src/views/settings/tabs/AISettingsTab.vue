<script setup lang="ts">
// AISettingsTab — advanced settings для Groq / local transcription setup.

import { computed, inject, onMounted, ref } from "vue";
import { SettingsList } from "@kosmos/visuals";
import { PhDownloadSimple, PhMicrophone, PhTextT } from "@phosphor-icons/vue";
import { ChevronRight, Cloud, ExternalLink, Plus, Trash2 } from "@lucide/vue";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import { DictationConfigKey } from "../composables/useDictationConfig";
import AddApiKeyModal from "./AddApiKeyModal.vue";
import ModelProviderBadge from "./ModelProviderBadge.vue";
import ProviderIcon from "./ProviderIcon.vue";
import type { AISettingsView } from "../navigation";

const props = defineProps<{
  intro: IntroDescriptor | null;
  view: AISettingsView;
}>();
const emit = defineEmits<{ "update:view": [view: AISettingsView] }>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("AISettingsTab requires DictationConfigKey provider in parent");

const {
  dictationHasApiKey,
  dictationApiKeyBusy,
  dictationLocalModelPath,
  dictationLocalModelId,
  dictationLocalModels,
  dictationLocalModelsBusy,
  dictationLocalModelsError,
  dictationLocalModelDownloadProgress,
  loadDictationConfig,
  onDictationClearApiKey,
  onDictationDeleteLocalModel,
  onDictationDownloadLocalModel,
} = ctx;

type ModelSortMode = "brand" | "speed" | "type";
type CatalogModelSource = "groq" | "local";

const apiKeyModalOpen = ref(false);
const modelSortMode = ref<ModelSortMode>("brand");
const modelSortMenuOpen = ref(false);

const GROQ_CONSOLE_URL = "https://console.groq.com/keys";

const activeSettingsView = computed<AISettingsView>({
  get: () => props.view,
  set: (value) => emit("update:view", value),
});

const activeIntro = computed(() => (activeSettingsView.value === "models" ? null : props.intro));
const modelSortOptions: Array<{ id: ModelSortMode; label: string }> = [
  { id: "brand", label: "По бренду" },
  { id: "speed", label: "По скорости" },
  { id: "type", label: "По типу" },
];

const modelSortLabel = computed(
  () => modelSortOptions.find((option) => option.id === modelSortMode.value)?.label ?? "По бренду",
);

const canAddApiKey = computed(() => !dictationHasApiKey.value);
const addApiKeyTitle = computed(() =>
  canAddApiKey.value ? "Добавить ключ" : "Ключ уже подключён. Чтобы заменить — удалите текущий.",
);

const localModelRows = computed(() =>
  (dictationLocalModels.value?.models ?? []).map((model) => ({
    ...model,
    uiState: localModelUiState(model),
  })),
);

type LocalCatalogModel = (typeof localModelRows.value)[number];

interface CatalogModelRow {
  id: string;
  name: string;
  typeLabel: string;
  speedScore: number;
  intelligenceScore: number;
  source: CatalogModelSource;
  localModel?: LocalCatalogModel;
}

const groqModelRows = computed<CatalogModelRow[]>(() => {
  if (!dictationHasApiKey.value) return [];

  return [
    {
      id: "groq-whisper-large-v3-turbo",
      name: "Whisper Large V3 Turbo",
      typeLabel: "Речь",
      speedScore: 0.86,
      intelligenceScore: 0.78,
      source: "groq",
    },
    {
      id: "groq-whisper-large-v3",
      name: "Whisper Large V3",
      typeLabel: "Речь",
      speedScore: 0.7,
      intelligenceScore: 0.88,
      source: "groq",
    },
    {
      id: "groq-gpt-oss-20b",
      name: "GPT OSS 20B",
      typeLabel: "Текст",
      speedScore: 1,
      intelligenceScore: 0.72,
      source: "groq",
    },
    {
      id: "groq-gpt-oss-120b",
      name: "GPT OSS 120B",
      typeLabel: "Текст",
      speedScore: 0.5,
      intelligenceScore: 0.9,
      source: "groq",
    },
    {
      id: "groq-llama-3-1-8b",
      name: "Llama 3.1 8B",
      typeLabel: "Текст",
      speedScore: 0.56,
      intelligenceScore: 0.62,
      source: "groq",
    },
    {
      id: "groq-llama-3-3-70b",
      name: "Llama 3.3 70B",
      typeLabel: "Текст",
      speedScore: 0.28,
      intelligenceScore: 0.82,
      source: "groq",
    },
    {
      id: "groq-qwen3-32b",
      name: "Qwen3-32B",
      typeLabel: "Текст",
      speedScore: 0.4,
      intelligenceScore: 0.8,
      source: "groq",
    },
    {
      id: "groq-llama-4-scout",
      name: "Llama 4 Scout 17B",
      typeLabel: "Текст",
      speedScore: 0.75,
      intelligenceScore: 0.78,
      source: "groq",
    },
  ];
});

const allCatalogModelRows = computed<CatalogModelRow[]>(() => [
  ...groqModelRows.value,
  ...localModelRows.value.map((model) => ({
    id: `local-${model.id}`,
    name: model.name,
    typeLabel: "Речь",
    speedScore: model.speedScore,
    intelligenceScore: model.accuracyScore,
    source: "local" as const,
    localModel: model,
  })),
]);

const modelCatalogRows = computed<CatalogModelRow[]>(() => {
  const rows = [...allCatalogModelRows.value];
  if (modelSortMode.value === "speed") {
    return rows.sort((a, b) => b.speedScore - a.speedScore || a.name.localeCompare(b.name));
  }

  if (modelSortMode.value === "type") {
    const typeRank = (row: CatalogModelRow) => (row.typeLabel === "Речь" ? 0 : 1);
    return rows.sort((a, b) => typeRank(a) - typeRank(b) || a.name.localeCompare(b.name));
  }

  return rows.sort((a, b) => a.name.localeCompare(b.name));
});

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

function formatModelSize(sizeMb: number | null | undefined): string {
  if (!Number.isFinite(sizeMb) || !sizeMb || sizeMb <= 0) return "0 Б";
  return formatBytes(sizeMb * 1024 * 1024);
}

function downloadProgressLabel(progress: {
  phase: string;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
}) {
  const phase =
    progress.phase === "runtime"
      ? "скачиваю STT runtime"
      : progress.phase === "tool"
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

function hasAnyLocalModelDownload() {
  return Object.keys(dictationLocalModelDownloadProgress.value).length > 0;
}

function localModelProgressPercent(modelId: string) {
  const progress = dictationLocalModelDownloadProgress.value[modelId];
  return Math.max(0, Math.min(100, progress?.percent ?? 0));
}

function scoreSegmentClass(score: number, index: number) {
  return index < Math.round(Math.max(0, Math.min(1, score)) * 5)
    ? "model-score__bar model-score__bar--active"
    : "model-score__bar";
}

function modelBadgeKind(model: CatalogModelRow): "groq" | "openai" | "nvidia" | "speech" | "text" {
  const lowerName = model.name.toLowerCase();
  if (lowerName.includes("parakeet")) return "nvidia";
  if (model.source === "local" && !lowerName.includes("whisper")) return "speech";
  if (lowerName.includes("gpt") || lowerName.includes("whisper") || model.source === "local") {
    return "openai";
  }
  if (model.source === "groq") return "groq";
  if (model.typeLabel === "Речь") return "speech";
  return "text";
}

function localModelUiState(model: {
  id: string;
  downloaded: boolean;
  selected: boolean;
  transcriptionSupported?: boolean;
}) {
  return {
    ...model,
    downloaded:
      model.selected ||
      (model.downloaded &&
        (model.transcriptionSupported === false ||
          Boolean(dictationLocalModels.value?.commandInstalled))),
  };
}

function onLocalModelAction(model: { id: string; downloaded: boolean; selected: boolean }) {
  const state = localModelUiState(model);
  if (hasAnyLocalModelDownload()) return;
  if (state.downloaded) {
    void onDictationDeleteLocalModel(model.id);
  } else {
    void onDictationDownloadLocalModel(model.id);
  }
}

function openModelsPage() {
  activeSettingsView.value = "models";
}

function openAddApiKeyModal() {
  if (!canAddApiKey.value) return;
  apiKeyModalOpen.value = true;
}

function onApiKeySaved() {
  void loadDictationConfig();
}

function openGroqConsole() {
  window.open(GROQ_CONSOLE_URL, "_blank", "noopener,noreferrer");
}

onMounted(() => {
  void loadDictationConfig();
});
</script>

<template>
  <AdvancedPageLayout
    :intro="activeIntro"
    :page-class="activeSettingsView === 'models' ? 'advanced-page--flush-top' : undefined"
  >
    <template v-if="activeSettingsView === 'overview'">
      <section class="advanced-page__section">
        <SettingsList>
          <button class="settings-nav-row" type="button" @click="openModelsPage">
            <span class="settings-nav-row__text">
              <strong>Модели</strong>
            </span>
            <ChevronRight :size="16" aria-hidden="true" />
          </button>
        </SettingsList>
      </section>

      <section class="advanced-page__section">
        <div class="api-keys-heading">
          <h2 class="advanced-section-title">API ключи</h2>
          <button
            type="button"
            class="api-keys-heading__add"
            :aria-label="addApiKeyTitle"
            :title="addApiKeyTitle"
            :disabled="!canAddApiKey"
            @click="openAddApiKeyModal"
          >
            <Plus :size="15" aria-hidden="true" />
          </button>
        </div>
        <div v-if="!dictationHasApiKey" class="api-keys-empty">Нет API ключей</div>
        <div v-else class="api-keys-list">
          <div class="api-key-item">
            <ProviderIcon provider="groq" :size="18" />
            <div class="api-key-item__main">
              <div class="api-key-item__provider">Groq</div>
            </div>
            <button
              type="button"
              class="api-key-item__icon-btn"
              aria-label="Удалить ключ"
              title="Удалить ключ"
              :disabled="dictationApiKeyBusy"
              @click="onDictationClearApiKey"
            >
              <Trash2 :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="api-key-item__icon-btn"
              aria-label="Открыть консоль Groq"
              title="Открыть консоль Groq"
              @click="openGroqConsole"
            >
              <ExternalLink :size="14" aria-hidden="true" />
            </button>
          </div>
        </div>
      </section>
    </template>

    <template v-else>
      <section class="advanced-page__section">
        <div class="model-manager-heading">
          <div class="model-sort">
            <button
              class="model-sort__button"
              type="button"
              :aria-expanded="modelSortMenuOpen"
              @click="modelSortMenuOpen = !modelSortMenuOpen"
            >
              Сортировать: {{ modelSortLabel }}
            </button>
            <div v-if="modelSortMenuOpen" class="model-sort__menu">
              <button
                v-for="option in modelSortOptions"
                :key="option.id"
                type="button"
                :class="{
                  'model-sort__item--active': modelSortMode === option.id,
                }"
                @click="
                  modelSortMode = option.id;
                  modelSortMenuOpen = false;
                "
              >
                {{ option.label }}
              </button>
            </div>
          </div>
          <button
            type="button"
            class="api-keys-heading__add"
            :aria-label="addApiKeyTitle"
            :title="addApiKeyTitle"
            :disabled="!canAddApiKey"
            @click="openAddApiKeyModal"
          >
            <Plus :size="15" aria-hidden="true" />
          </button>
        </div>

        <p v-if="dictationLocalModelsError" class="model-error">
          {{ dictationLocalModelsError }}
        </p>

        <div class="model-table">
          <div class="model-table__head">
            <span>Модель</span>
            <span class="model-table__head-right">Тип</span>
            <span class="model-table__head-right">Скорость / качество</span>
            <span class="model-table__head-status">Онлайн / оффлайн</span>
          </div>
          <div v-for="model in modelCatalogRows" :key="model.id" class="model-row">
            <div class="model-name">
              <ModelProviderBadge :kind="modelBadgeKind(model)" />
              <strong>{{ model.name }}</strong>
            </div>
            <span class="model-type-icon" :title="model.typeLabel" :aria-label="model.typeLabel">
              <PhMicrophone
                v-if="model.typeLabel === 'Речь'"
                :size="15"
                weight="bold"
                aria-hidden="true"
              />
              <PhTextT v-else :size="15" weight="bold" aria-hidden="true" />
            </span>
            <div class="model-score-pair" aria-label="Скорость и качество">
              <div class="model-score" aria-label="Скорость">
                <span
                  v-for="index in 5"
                  :key="`speed-${model.id}-${index}`"
                  :class="scoreSegmentClass(model.speedScore, index)"
                />
              </div>
              <div class="model-score model-score--secondary" aria-label="Качество">
                <span
                  v-for="index in 5"
                  :key="`quality-${model.id}-${index}`"
                  :class="scoreSegmentClass(model.intelligenceScore, index)"
                />
              </div>
            </div>
            <div v-if="model.source === 'groq'" class="model-status model-status--online">
              <Cloud class="model-status__cloud" :size="13" aria-hidden="true" />
            </div>
            <div v-else-if="model.localModel" class="model-status model-status--offline">
              <span class="model-status__size">
                {{ formatModelSize(model.localModel.sizeMb) }}
              </span>
              <div class="local-model-actions">
                <button
                  v-if="!model.localModel.uiState.downloaded"
                  class="model-icon-button model-icon-button--download"
                  type="button"
                  :disabled="Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload()"
                  aria-label="Скачать модель"
                  @click="onLocalModelAction(model.localModel)"
                >
                  <PhDownloadSimple :size="12" weight="bold" aria-hidden="true" />
                </button>
                <button
                  v-else
                  class="model-icon-button model-icon-button--delete"
                  type="button"
                  aria-label="Удалить модель"
                  :disabled="Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload()"
                  @click="onLocalModelAction(model.localModel)"
                >
                  <Trash2 :size="11" aria-hidden="true" />
                </button>
                <div
                  v-if="dictationLocalModelDownloadProgress[model.localModel.id]"
                  class="local-model-progress"
                  :class="{
                    'local-model-progress--indeterminate':
                      dictationLocalModelDownloadProgress[model.localModel.id].percent === null,
                  }"
                  role="progressbar"
                  :aria-valuemin="0"
                  :aria-valuemax="100"
                  :aria-valuenow="localModelProgressPercent(model.localModel.id)"
                  :aria-label="
                    downloadProgressLabel(dictationLocalModelDownloadProgress[model.localModel.id])
                  "
                >
                  <div
                    class="local-model-progress-fill"
                    :style="{
                      width: `${localModelProgressPercent(model.localModel.id)}%`,
                    }"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </template>

    <AddApiKeyModal
      :open="apiKeyModalOpen"
      @close="apiKeyModalOpen = false"
      @saved="onApiKeySaved"
    />
  </AdvancedPageLayout>
</template>

<style scoped>
.settings-nav-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 14px;
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
  gap: 2px;
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

.settings-nav-row__text small {
  overflow: hidden;
  color: color-mix(in srgb, currentColor 56%, transparent);
  font-size: 11px;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-keys-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.api-keys-heading .advanced-section-title {
  margin-bottom: 0;
}

.api-keys-heading__add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  color: color-mix(in srgb, currentColor 65%, transparent);
  background: transparent;
}

.api-keys-heading__add:hover:not(:disabled) {
  color: inherit;
  background: color-mix(in srgb, currentColor 8%, transparent);
}

.api-keys-heading__add:disabled {
  cursor: default;
  opacity: 0.35;
}

.api-keys-empty {
  display: flex;
  align-items: center;
  min-height: 48px;
  padding: 10px 12px;
  border: 1px dashed color-mix(in srgb, currentColor 15%, transparent);
  border-radius: 8px;
  color: color-mix(in srgb, currentColor 52%, transparent);
  background: color-mix(in srgb, currentColor 3%, transparent);
  font-size: 12px;
}

.api-keys-list {
  display: grid;
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 4%, transparent);
  overflow: hidden;
}

.api-key-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  gap: 10px;
  align-items: center;
  min-height: 48px;
  padding: 10px 12px;
}

.api-key-item__main {
  min-width: 0;
}

.api-key-item__provider {
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-key-item__icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  color: color-mix(in srgb, currentColor 62%, transparent);
  background: transparent;
}

.api-key-item__icon-btn:hover:not(:disabled) {
  color: inherit;
  background: color-mix(in srgb, currentColor 8%, transparent);
}

.api-key-item__icon-btn:disabled {
  cursor: default;
  opacity: 0.45;
}

.model-manager-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 16px;
}

.model-manager-heading .advanced-section-title {
  margin-bottom: 2px;
}

.model-manager-heading p {
  max-width: 440px;
  margin: 0;
  color: color-mix(in srgb, currentColor 56%, transparent);
  font-size: 12px;
  line-height: 1.35;
}

.model-sort {
  position: relative;
  flex: 0 0 auto;
}

.model-sort__button {
  height: 30px;
  padding: 0 10px;
  border: 1px solid color-mix(in srgb, currentColor 16%, transparent);
  border-radius: 7px;
  color: inherit;
  background: color-mix(in srgb, currentColor 5%, transparent);
  font: inherit;
  font-size: 12px;
  font-weight: 650;
  cursor: pointer;
}

.model-sort__button:hover {
  background: color-mix(in srgb, currentColor 8%, transparent);
}

.model-sort__menu {
  position: absolute;
  z-index: 10;
  top: calc(100% + 8px);
  right: 0;
  display: grid;
  min-width: 180px;
  padding: 6px;
  border: 1px solid color-mix(in srgb, currentColor 18%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--background, #1f1f1f) 92%, currentColor);
  box-shadow: 0 12px 32px color-mix(in srgb, #000 38%, transparent);
}

.model-sort__menu button {
  height: 30px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  color: inherit;
  background: transparent;
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

.model-sort__menu button:hover,
.model-sort__item--active {
  background: color-mix(in srgb, currentColor 10%, transparent);
}

.model-table {
  --model-type-column-width: 24px;
  --model-score-column-width: 108px;
  --model-status-column-width: 88px;
  display: grid;
  gap: 6px;
  padding: 0;
}

.model-table__head,
.model-row {
  display: grid;
  grid-template-columns:
    minmax(0, 1fr)
    var(--model-type-column-width)
    var(--model-score-column-width)
    var(--model-status-column-width);
  gap: 8px;
  align-items: center;
}

.model-table__head {
  min-height: 22px;
  padding: 0 5px;
  color: color-mix(in srgb, currentColor 48%, transparent);
  font-size: 10px;
  font-weight: 700;
}

.model-table__head > span {
  justify-self: start;
  text-align: left;
}

.model-table__head-right {
  justify-self: end !important;
  text-align: right !important;
}

.model-table__head-status {
  justify-self: end !important;
  text-align: right !important;
}

.model-row {
  min-height: 30px;
  padding: 0 5px;
}

.model-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.model-name strong {
  min-width: 0;
  overflow: hidden;
  color: color-mix(in srgb, currentColor 82%, transparent);
  font-size: 12px;
  font-weight: 450;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-score {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 2px;
  width: 100%;
}

.model-score-pair {
  display: grid;
  grid-template-columns: 1fr;
  justify-items: end;
  gap: 4px;
  min-width: 0;
  width: var(--model-score-column-width);
  max-width: 100%;
  justify-self: end;
}

.model-score--secondary {
  opacity: 0.88;
}

.model-score__bar {
  height: 2px;
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 12%, transparent);
}

.model-score__bar--active {
  background: color-mix(in srgb, currentColor 86%, transparent);
}

.model-type-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  justify-self: end;
  color: color-mix(in srgb, currentColor 62%, transparent);
}

.model-status {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  min-width: 0;
  width: 100%;
  justify-self: end;
}

.model-status--online {
  gap: 0;
}

.model-status--offline {
  gap: 8px;
}

.model-status__size {
  color: color-mix(in srgb, currentColor 48%, transparent);
  font-size: 11px;
  line-height: 1.2;
  text-align: right;
  white-space: nowrap;
}

.model-status__cloud {
  flex: 0 0 auto;
  color: color-mix(in srgb, currentColor 62%, transparent);
}

.model-error {
  margin: 0;
  padding: 0 12px 10px;
  color: #ff6b6b;
  font-size: 12px;
}

.local-model-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 2px;
  min-width: 20px;
}

.model-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 4px;
  color: inherit;
  background: color-mix(in srgb, currentColor 6%, transparent);
  font: inherit;
  font-size: 9px;
  font-weight: 650;
  cursor: pointer;
}

.model-icon-button:hover:not(:disabled) {
  background: color-mix(in srgb, currentColor 10%, transparent);
}

.model-icon-button--download,
.model-icon-button--delete {
  cursor: default;
}

.model-icon-button:disabled {
  cursor: default;
  opacity: 0.45;
}

.local-model-progress {
  width: 28px;
  height: 2px;
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

.local-model-progress--indeterminate .local-model-progress-fill {
  width: 42% !important;
  min-width: 42%;
  animation: local-model-progress-indeterminate 1.1s ease-in-out infinite;
}

@keyframes local-model-progress-indeterminate {
  0% {
    transform: translateX(-110%);
  }

  100% {
    transform: translateX(245%);
  }
}
</style>
