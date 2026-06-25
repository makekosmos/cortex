// useDictationConfig — единый state + handlers для диктации и AI settings.
// Используется несколькими tab'ами SettingsView'а, поэтому composable должен
// быть SINGLETON на уровне owner'а — иначе копии state разойдутся. Реализуем
// через `provide`/`inject` (DictationConfigKey).

import { computed, type InjectionKey, ref } from "vue";
import {
  buildDictationGroqStatus,
  buildDictationLocalStatus,
  buildDictationMicOptions,
  buildDictationVoiceModelOptions,
  buildDictationVoiceModelValue,
  buildStatsCards,
  normalizeDictationConfig,
  normalizeDictationStats,
  type ConnectivityReport,
  type DictationConfigData,
  type DictationLocalModelsSnapshot,
  type DictationStatsData,
  type DictationVoiceModelOption,
} from "./useDictationConfig.shared";
import { createDictationConfigActions } from "./useDictationConfig.handlers";

export {
  DEFAULT_DICTATION_CFG,
  DEFAULT_DICTATION_STATS,
  DICTATION_INJECT_OPTIONS,
  DICTATION_LANGUAGE_OPTIONS,
  DICTATION_PROVIDER_OPTIONS,
  DICTATION_TRIGGER_OPTIONS,
  DNS_PROFILE_OPTIONS,
  buildAccelerator,
} from "./useDictationConfig.shared";
export type {
  ConnectivityReport,
  DictationConfigData,
  DictationLocalModelsSnapshot,
  DictationStatsData,
  DictationVoiceModelOption,
  DnsKind,
} from "./useDictationConfig.shared";

export function createDictationConfig() {
  const dictationConfig = ref<DictationConfigData>(normalizeDictationConfig());
  const dictationStats = ref<DictationStatsData>(normalizeDictationStats());
  const dictationHasApiKey = ref(false);
  const dictationApiKeyInput = ref("");
  const dictationApiKeyBusy = ref(false);
  const dictationApiKeyMsg = ref("");
  const dictationCustomDohUrl = ref("");
  const dictationConnTestBusy = ref(false);
  const dictationConnTestResult = ref<string>("");
  const dictationConnReport = ref<ConnectivityReport | null>(null);
  const dictationCustomDohError = ref<string | null>(null);
  const dictationMicDevices = ref<{ deviceId: string; label: string }[]>([]);
  const dictationMicError = ref<string>("");
  const dictationLocalModels = ref<DictationLocalModelsSnapshot | null>(null);
  const dictationLocalModelsBusy = ref<string | null>(null);
  const dictationLocalModelsError = ref<string>("");
  const dictationLocalModelDownloadProgress = ref<
    Record<
      string,
      {
        phase: string;
        downloadedBytes: number;
        totalBytes: number | null;
        percent: number | null;
      }
    >
  >({});

  const dictationCaptureAccelerator = ref<string | null>(null);
  const dictationCaptureCancelTick = ref(0);

  const dictationMicOptions = computed(() => buildDictationMicOptions(dictationMicDevices.value));
  const statsCards = computed(() => buildStatsCards(dictationStats.value));
  const dictationModelName = computed<string>({
    get: () => dictationConfig.value.model,
    set: (value) => {
      dictationConfig.value.model = value;
    },
  });
  const dictationLocalModelPath = computed<string>({
    get: () => dictationConfig.value.localModelPath ?? "",
    set: (value) => {
      dictationConfig.value.localModelPath = value;
    },
  });
  const dictationLocalModelId = computed<string>({
    get: () => dictationConfig.value.localModelId,
    set: (value) => {
      dictationConfig.value.localModelId = value;
    },
  });
  const dictationLocalEngine = computed(() => dictationConfig.value.localEngine);
  const dictationVoiceModelOptions = computed<DictationVoiceModelOption[]>(() =>
    buildDictationVoiceModelOptions({
      hasApiKey: dictationHasApiKey.value,
      localModels: dictationLocalModels.value,
    }),
  );
  const dictationVoiceModelValue = computed(() =>
    buildDictationVoiceModelValue(dictationConfig.value),
  );
  const dictationGroqStatus = computed(() =>
    buildDictationGroqStatus(dictationConfig.value.provider, dictationHasApiKey.value),
  );
  const dictationLocalStatus = computed(() =>
    buildDictationLocalStatus(
      dictationConfig.value.provider,
      dictationLocalModelPath.value,
      dictationLocalModelId.value,
    ),
  );
  const dictationProviderDescription = computed(() => {
    switch (dictationConfig.value.provider) {
      case "local":
        return dictationLocalStatus.value;
      case "mock":
        return "Тестовый режим: mock provider используется только для автоматических smoke-прогонов.";
      default:
        return dictationGroqStatus.value;
    }
  });

  const actions = createDictationConfigActions({
    dictationConfig,
    dictationStats,
    dictationHasApiKey,
    dictationApiKeyInput,
    dictationApiKeyBusy,
    dictationApiKeyMsg,
    dictationCustomDohUrl,
    dictationConnTestBusy,
    dictationConnTestResult,
    dictationConnReport,
    dictationCustomDohError,
    dictationMicDevices,
    dictationMicError,
    dictationLocalModels,
    dictationLocalModelsBusy,
    dictationLocalModelsError,
    dictationLocalModelDownloadProgress,
    dictationCaptureAccelerator,
    dictationCaptureCancelTick,
  });

  return {
    dictationConfig,
    dictationStats,
    dictationHasApiKey,
    dictationApiKeyInput,
    dictationApiKeyBusy,
    dictationApiKeyMsg,
    dictationCustomDohUrl,
    dictationModelName,
    dictationLocalModelPath,
    dictationLocalModelId,
    dictationLocalEngine,
    dictationVoiceModelOptions,
    dictationVoiceModelValue,
    dictationLocalModels,
    dictationLocalModelsBusy,
    dictationLocalModelsError,
    dictationLocalModelDownloadProgress,
    dictationConnTestBusy,
    dictationConnTestResult,
    dictationConnReport,
    dictationCustomDohError,
    dictationMicError,
    dictationCaptureAccelerator,
    dictationCaptureCancelTick,
    dictationProviderDescription,
    dictationMicOptions,
    statsCards,
    dictationGroqStatus,
    dictationLocalStatus,
    ...actions,
  };
}

export type DictationConfigCtx = ReturnType<typeof createDictationConfig>;

export const DictationConfigKey: InjectionKey<DictationConfigCtx> = Symbol("DictationConfigKey");
