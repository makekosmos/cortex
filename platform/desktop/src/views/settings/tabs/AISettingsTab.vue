<script setup lang="ts">
// AISettingsTab — advanced settings для Groq / local transcription setup.

import { computed, inject, onMounted, ref } from "vue";
import { SettingsList } from "@kosmos/visuals";
import { PhDownloadSimple, PhMicrophone, PhOpenAiLogo, PhTextT } from "@phosphor-icons/vue";
import { Boxes, ChevronRight, ExternalLink, Plus, Trash2 } from "@lucide/vue";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import { DictationConfigKey } from "../composables/useDictationConfig";
import AddApiKeyModal from "./AddApiKeyModal.vue";
import ProviderIcon from "./ProviderIcon.vue";

const props = defineProps<{
  intro: IntroDescriptor | null;
}>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("AISettingsTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationHasApiKey,
  dictationApiKeyBusy,
  dictationModelName,
  dictationLocalModelPath,
  dictationLocalModelId,
  dictationLocalEngine,
  dictationLocalModels,
  dictationLocalModelsBusy,
  dictationLocalModelsError,
  dictationLocalModelDownloadProgress,
  loadDictationConfig,
  onDictationClearApiKey,
  onDictationDeleteLocalModel,
  onDictationDownloadLocalModel,
} = ctx;

type AISettingsView = "overview" | "models";
type ModelSortMode = "brand" | "speed" | "type";
type CatalogModelSource = "groq" | "local";

const activeSettingsView = ref<AISettingsView>("overview");
const apiKeyModalOpen = ref(false);
const modelSortMode = ref<ModelSortMode>("brand");
const modelSortMenuOpen = ref(false);

const GROQ_CONSOLE_URL = "https://console.groq.com/keys";

const modelsIntro = computed<IntroDescriptor>(() => ({
  icon: Boxes,
  label: "Модели",
  description: "Выберите модели для Kosmos AI.",
  iconGradient: props.intro?.iconGradient,
}));

const activeIntro = computed(() =>
  activeSettingsView.value === "models" ? modelsIntro.value : props.intro,
);

const isGroqProvider = computed(() => dictationConfig.value.provider === "groq");
const isFasterWhisperEngine = computed(() => {
  const engine = dictationLocalEngine.value.trim().toLowerCase();
  return engine === "faster-whisper" || engine === "faster_whisper";
});

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
  brand: string;
  typeLabel: string;
  speedScore: number;
  intelligenceScore: number;
  statusLabel: string;
  selected: boolean;
  source: CatalogModelSource;
  localModel?: LocalCatalogModel;
}

interface CatalogModelSection {
  id: string;
  name: string;
  description?: string;
  rows: CatalogModelRow[];
}

const groqModelRows = computed<CatalogModelRow[]>(() => {
  if (!dictationHasApiKey.value) return [];

  const selectedModel = dictationModelName.value.trim() || "whisper-large-v3";

  return [
    {
      id: "groq-whisper-large-v3-turbo",
      name: "Whisper Large V3 Turbo",
      brand: "Groq",
      typeLabel: "Речь",
      speedScore: 0.86,
      intelligenceScore: 0.78,
      statusLabel: "",
      selected: isGroqProvider.value && selectedModel === "whisper-large-v3-turbo",
      source: "groq",
    },
    {
      id: "groq-whisper-large-v3",
      name: "Whisper Large V3",
      brand: "Groq",
      typeLabel: "Речь",
      speedScore: 0.7,
      intelligenceScore: 0.88,
      statusLabel: "",
      selected: isGroqProvider.value && selectedModel === "whisper-large-v3",
      source: "groq",
    },
    {
      id: "groq-gpt-oss-20b",
      name: "GPT OSS 20B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 1,
      intelligenceScore: 0.72,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
    {
      id: "groq-gpt-oss-120b",
      name: "GPT OSS 120B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 0.5,
      intelligenceScore: 0.9,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
    {
      id: "groq-llama-3-1-8b",
      name: "Llama 3.1 8B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 0.56,
      intelligenceScore: 0.62,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
    {
      id: "groq-llama-3-3-70b",
      name: "Llama 3.3 70B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 0.28,
      intelligenceScore: 0.82,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
    {
      id: "groq-qwen3-32b",
      name: "Qwen3-32B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 0.4,
      intelligenceScore: 0.8,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
    {
      id: "groq-llama-4-scout",
      name: "Llama 4 Scout 17B",
      brand: "Groq",
      typeLabel: "Текст",
      speedScore: 0.75,
      intelligenceScore: 0.78,
      statusLabel: "Скоро",
      selected: false,
      source: "groq",
    },
  ];
});

const allCatalogModelRows = computed<CatalogModelRow[]>(() => [
  ...groqModelRows.value,
  ...localModelRows.value.map((model) => ({
    id: `local-${model.id}`,
    name: model.name,
    brand: "OpenAI",
    typeLabel: "Речь",
    speedScore: model.speedScore,
    intelligenceScore: model.accuracyScore,
    statusLabel: model.uiState.selected
      ? "Используется"
      : model.uiState.downloaded
        ? "Готово"
        : "Не готово",
    selected: model.uiState.selected,
    source: "local" as const,
    localModel: model,
  })),
]);

const modelCatalogSections = computed<CatalogModelSection[]>(() => {
  const rows = [...allCatalogModelRows.value];
  if (modelSortMode.value === "speed") {
    return [
      {
        id: "speed",
        name: "По скорости",
        rows: rows.sort((a, b) => b.speedScore - a.speedScore || a.name.localeCompare(b.name)),
      },
    ];
  }

  const groupKey =
    modelSortMode.value === "type"
      ? (row: CatalogModelRow) => row.typeLabel
      : (row: CatalogModelRow) => row.brand;
  const groups = new Map<string, CatalogModelRow[]>();
  for (const row of rows) {
    const key = groupKey(row);
    groups.set(key, [...(groups.get(key) ?? []), row]);
  }

  return [...groups.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, groupRows]) => ({
      id: `${modelSortMode.value}-${name}`,
      name,
      rows: groupRows.sort((a, b) => a.name.localeCompare(b.name)),
    }));
});

function fasterWhisperModelId(modelId: string) {
  switch (modelId) {
    case "tiny-q5_1":
      return "tiny";
    case "turbo":
      return "large-v3-turbo";
    case "large":
      return "large-v3";
    default:
      return modelId;
  }
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
        : progress.phase === "python"
          ? "создаю Python env"
          : progress.phase === "package"
            ? "ставлю faster-whisper"
            : progress.phase === "faster-whisper"
              ? "готовлю Faster Whisper"
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

function localModelUiState(model: { id: string; downloaded: boolean; selected: boolean }) {
  if (!isFasterWhisperEngine.value) return model;
  const backendModelId = fasterWhisperModelId(model.id);
  const selected =
    dictationLocalModelId.value === model.id && dictationLocalModelPath.value === backendModelId;
  return {
    ...model,
    downloaded: model.downloaded || selected,
    selected,
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
  <AdvancedPageLayout :intro="activeIntro">
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
          <div>
            <h2 class="advanced-section-title">Управление моделями</h2>
          </div>
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
                :class="{ 'model-sort__item--active': modelSortMode === option.id }"
                @click="
                  modelSortMode = option.id;
                  modelSortMenuOpen = false;
                "
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>

        <p v-if="dictationLocalModelsError" class="model-error">{{ dictationLocalModelsError }}</p>

        <div class="model-catalog">
          <section v-for="section in modelCatalogSections" :key="section.id" class="model-provider">
            <header class="model-provider__header">
              <div class="model-provider__title">
                <span
                  v-if="section.name === 'OpenAI'"
                  class="model-provider__icon"
                  aria-hidden="true"
                >
                  <PhOpenAiLogo :size="16" weight="bold" />
                </span>
                <span
                  v-else-if="section.name === 'Groq'"
                  class="model-provider__icon model-provider__icon--groq"
                  aria-hidden="true"
                />
                <h3>{{ section.name }}</h3>
              </div>
            </header>
            <div class="model-table">
              <div class="model-table__head">
                <span>Модель</span>
                <span>Скорость</span>
                <span>Качество</span>
                <span>Тип</span>
                <span />
              </div>
              <div v-for="model in section.rows" :key="model.id" class="model-row">
                <div class="model-name">
                  <strong>{{ model.name }}</strong>
                </div>
                <div class="model-score" aria-label="Скорость">
                  <span
                    v-for="index in 5"
                    :key="`speed-${model.id}-${index}`"
                    :class="scoreSegmentClass(model.speedScore, index)"
                  />
                </div>
                <div class="model-score" aria-label="Качество">
                  <span
                    v-for="index in 5"
                    :key="`quality-${model.id}-${index}`"
                    :class="scoreSegmentClass(model.intelligenceScore, index)"
                  />
                </div>
                <span
                  class="model-type-icon"
                  :title="model.typeLabel"
                  :aria-label="model.typeLabel"
                >
                  <PhMicrophone
                    v-if="model.typeLabel === 'Речь'"
                    :size="15"
                    weight="bold"
                    aria-hidden="true"
                  />
                  <PhTextT v-else :size="15" weight="bold" aria-hidden="true" />
                </span>
                <div
                  v-if="model.source === 'local' && model.localModel"
                  class="local-model-actions"
                >
                  <button
                    v-if="!model.localModel.uiState.downloaded"
                    class="model-icon-button model-icon-button--download"
                    type="button"
                    :disabled="Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload()"
                    aria-label="Скачать модель"
                    @click="onLocalModelAction(model.localModel)"
                  >
                    <PhDownloadSimple :size="15" weight="bold" aria-hidden="true" />
                  </button>
                  <button
                    v-else
                    class="model-icon-button"
                    type="button"
                    aria-label="Удалить модель"
                    :disabled="Boolean(dictationLocalModelsBusy) || hasAnyLocalModelDownload()"
                    @click="onLocalModelAction(model.localModel)"
                  >
                    <Trash2 :size="14" aria-hidden="true" />
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
                      downloadProgressLabel(
                        dictationLocalModelDownloadProgress[model.localModel.id],
                      )
                    "
                  >
                    <div
                      class="local-model-progress-fill"
                      :style="{ width: `${localModelProgressPercent(model.localModel.id)}%` }"
                    />
                  </div>
                </div>
                <span v-else class="model-checkbox-cell">
                  <input
                    class="model-checkbox"
                    type="checkbox"
                    aria-label="Показать модель в списках"
                    @click.prevent
                  />
                </span>
              </div>
            </div>
          </section>
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

.model-catalog {
  display: grid;
  gap: 10px;
}

.model-provider {
  overflow: hidden;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 4%, transparent);
}

.model-provider__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 12px 10px;
}

.model-provider__title {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  gap: 8px;
}

.model-provider__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  color: color-mix(in srgb, currentColor 78%, transparent);
}

.model-provider__icon--groq::before {
  content: "";
  width: 15px;
  height: 15px;
  background: currentColor;
  mask: url("../../../assets/providers/groq.svg") center / contain no-repeat;
  -webkit-mask: url("../../../assets/providers/groq.svg") center / contain no-repeat;
}

.model-provider__header h3 {
  margin: 0;
  font-size: 15px;
  line-height: 1.25;
}

.model-provider__header p {
  max-width: 560px;
  margin: 5px 0 0;
  color: color-mix(in srgb, currentColor 62%, transparent);
  font-size: 12px;
  line-height: 1.35;
}

.model-checkbox-cell {
  display: inline-flex;
  align-items: center;
  justify-content: flex-start;
  width: 28px;
  height: 28px;
}

.model-checkbox {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: currentColor;
}

.model-table {
  display: grid;
  gap: 6px;
  padding: 0 10px 10px;
}

.model-table__head,
.model-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 66px 66px 32px 32px;
  gap: 8px;
  align-items: center;
}

.model-table__head {
  min-height: 28px;
  padding: 0 8px;
  color: color-mix(in srgb, currentColor 48%, transparent);
  font-size: 11px;
  font-weight: 700;
}

.model-table__head > span {
  justify-self: start;
  text-align: left;
}

.model-row {
  min-height: 42px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, currentColor 7%, transparent);
  border-radius: 7px;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.model-name {
  display: grid;
  min-width: 0;
}

.model-name strong {
  overflow: hidden;
  color: color-mix(in srgb, currentColor 82%, transparent);
  font-size: 11px;
  font-weight: 450;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-score {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 2px;
  width: 66px;
}

.model-score__bar {
  height: 3px;
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
  width: 28px;
  height: 28px;
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
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-start;
  gap: 4px;
  width: 28px;
}

.model-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 6px;
  color: inherit;
  background: color-mix(in srgb, currentColor 6%, transparent);
  font: inherit;
  font-size: 11px;
  font-weight: 650;
  cursor: pointer;
}

.model-icon-button:hover:not(:disabled) {
  background: color-mix(in srgb, currentColor 10%, transparent);
}

.model-icon-button--download {
  cursor: default;
}

.model-icon-button:disabled {
  cursor: default;
  opacity: 0.45;
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
