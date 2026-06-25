import type { Ref } from "vue";
import {
  applyDictationConfigSnapshot,
  DEFAULT_DICTATION_CFG,
  normalizeDictationStats,
  type DictationConfigData,
  type DictationLocalModelsSnapshot,
  type DictationProvider,
  type DictationStatsData,
} from "./useDictationConfig.shared";
import { createDictationKeyActions } from "./useDictationConfig.keys";

type DictationLocalModelDownloadProgress = Record<
  string,
  { phase: string; downloadedBytes: number; totalBytes: number | null; percent: number | null }
>;

interface DictationConfigActionsArgs {
  dictationConfig: Ref<DictationConfigData>;
  dictationStats: Ref<DictationStatsData>;
  dictationHasApiKey: Ref<boolean>;
  dictationApiKeyInput: Ref<string>;
  dictationApiKeyBusy: Ref<boolean>;
  dictationApiKeyMsg: Ref<string>;
  dictationCustomDohUrl: Ref<string>;
  dictationConnTestBusy: Ref<boolean>;
  dictationConnTestResult: Ref<string>;
  dictationConnReport: Ref<import("./useDictationConfig.shared").ConnectivityReport | null>;
  dictationCustomDohError: Ref<string | null>;
  dictationMicDevices: Ref<{ deviceId: string; label: string }[]>;
  dictationMicError: Ref<string>;
  dictationLocalModels: Ref<DictationLocalModelsSnapshot | null>;
  dictationLocalModelsBusy: Ref<string | null>;
  dictationLocalModelsError: Ref<string>;
  dictationLocalModelDownloadProgress: Ref<DictationLocalModelDownloadProgress>;
  dictationCaptureAccelerator: Ref<string | null>;
  dictationCaptureCancelTick: Ref<number>;
}

export function createDictationConfigActions(args: DictationConfigActionsArgs) {
  let dictationArkUnsubscribe: (() => void) | null = null;

  function ensureDictationEventSubscription() {
    if (dictationArkUnsubscribe) return;
    dictationArkUnsubscribe = window.kepler.ark.onEvent((event) => {
      const kind = event.event;
      const modelId = typeof event.modelId === "string" ? event.modelId : "";
      if (!modelId) return;

      if (kind === "dictation_local_model_download_started") {
        args.dictationLocalModelsError.value = "";
        args.dictationLocalModelDownloadProgress.value = {
          ...args.dictationLocalModelDownloadProgress.value,
          [modelId]: {
            phase: "model",
            downloadedBytes: 0,
            totalBytes: null,
            percent: null,
          },
        };
        return;
      }

      if (kind === "dictation_local_model_download_progress") {
        args.dictationLocalModelDownloadProgress.value = {
          ...args.dictationLocalModelDownloadProgress.value,
          [modelId]: {
            phase: typeof event.phase === "string" ? event.phase : "download",
            downloadedBytes: typeof event.downloadedBytes === "number" ? event.downloadedBytes : 0,
            totalBytes: typeof event.totalBytes === "number" ? event.totalBytes : null,
            percent: typeof event.percent === "number" ? event.percent : null,
          },
        };
        return;
      }

      if (kind === "dictation_local_model_download_complete") {
        applyDictationConfigSnapshot(
          event.config && typeof event.config === "object"
            ? (event.config as Partial<DictationConfigData>)
            : null,
          args.dictationConfig,
          args.dictationCustomDohUrl,
        );
        if (event.localModels && typeof event.localModels === "object") {
          args.dictationLocalModels.value = event.localModels as DictationLocalModelsSnapshot;
        } else {
          void loadDictationLocalModels();
        }
        const next = { ...args.dictationLocalModelDownloadProgress.value };
        delete next[modelId];
        args.dictationLocalModelDownloadProgress.value = next;
        return;
      }

      if (kind === "dictation_local_model_download_failed") {
        const next = { ...args.dictationLocalModelDownloadProgress.value };
        delete next[modelId];
        args.dictationLocalModelDownloadProgress.value = next;
        args.dictationLocalModelsError.value =
          typeof event.error === "string" ? event.error : "Скачивание не удалось.";
      }
    });
  }

  async function loadDictationLocalModels() {
    args.dictationLocalModelsError.value = "";
    try {
      args.dictationLocalModels.value = (await window.kepler.ark.request(
        "dictation.list_local_models",
        {},
      )) as DictationLocalModelsSnapshot;
    } catch (e) {
      args.dictationLocalModelsError.value = (e as Error).message;
    }
  }

  async function loadDictationConfig() {
    ensureDictationEventSubscription();
    try {
      const resp = (await window.kepler.ark.request("dictation.get_config", {})) as {
        config?: Partial<DictationConfigData>;
        hasApiKey?: boolean;
      };
      if (resp.config) {
        applyDictationConfigSnapshot(resp.config, args.dictationConfig, args.dictationCustomDohUrl);
      }
      args.dictationHasApiKey.value = resp.hasApiKey ?? false;
      await loadDictationLocalModels();
    } catch (e) {
      console.error("[settings] loadDictationConfig failed:", e);
    }
  }

  async function loadDictationStats() {
    try {
      const resp = (await window.kepler.ark.request(
        "dictation.get_stats",
        {},
      )) as Partial<DictationStatsData>;
      args.dictationStats.value = normalizeDictationStats(resp);
    } catch (e) {
      console.error("[settings] loadDictationStats failed:", e);
    }
  }

  async function loadDictationMicrophones() {
    args.dictationMicError.value = "";
    try {
      try {
        const probe = await navigator.mediaDevices.getUserMedia({ audio: true });
        for (const track of probe.getTracks()) track.stop();
      } catch (permErr) {
        args.dictationMicError.value =
          "Нет доступа к микрофону — выберите устройство после первого запуска записи.";
        console.warn("[settings] mic permission probe failed:", permErr);
      }
      const devices = await navigator.mediaDevices.enumerateDevices();
      args.dictationMicDevices.value = devices
        .filter((d) => d.kind === "audioinput")
        .map((d, i) => ({
          deviceId: d.deviceId,
          label: d.label || `Микрофон ${i + 1}`,
        }));
    } catch (e) {
      console.error("[settings] loadDictationMicrophones failed:", e);
      args.dictationMicError.value = `Ошибка: ${(e as Error).message}`;
    }
  }

  async function patchDictationConfig(patch: Record<string, unknown>) {
    try {
      await window.kepler.ark.request("dictation.update_config", patch);
      await loadDictationConfig();
    } catch (e) {
      console.error("[settings] update_config failed:", e);
    }
  }

  async function onDictationMicChange(v: string) {
    const next = v && v !== "default" ? v : null;
    await patchDictationConfig({ microphoneDeviceId: next });
  }

  async function onDictationLanguageChange(v: string) {
    await patchDictationConfig({ language: v });
  }

  async function onDictationInjectModeChange(v: "auto_paste" | "clipboard_only") {
    await patchDictationConfig({ injectMode: v });
  }

  async function onDictationTriggerModeChange(v: "toggle" | "push_to_talk") {
    await patchDictationConfig({ triggerMode: v });
  }

  async function onDictationDuckAudioChange(enabled: boolean) {
    await patchDictationConfig({ duckAudioDuringRecording: enabled });
  }

  async function onDictationProviderChange(v: DictationProvider) {
    await patchDictationConfig({ provider: v });
  }

  async function onDictationVoiceModelChange(value: string) {
    const [source, modelId] = value.split(":", 2);
    if (!modelId) return;
    if (source === "groq") {
      await patchDictationConfig({
        provider: "groq",
        providerEnabled: true,
        model: modelId,
      });
      return;
    }
    if (source === "local") {
      await onDictationUseLocalModel(modelId);
    }
  }

  async function onDictationProviderEnabledChange(enabled: boolean) {
    await patchDictationConfig({ providerEnabled: enabled });
  }

  async function onDictationModelBlur() {
    const model = args.dictationConfig.value.model.trim();
    await patchDictationConfig({ model: model || DEFAULT_DICTATION_CFG.model });
  }

  async function onDictationDownloadLocalModel(modelId: string) {
    args.dictationLocalModelsBusy.value = modelId;
    args.dictationLocalModelsError.value = "";
    args.dictationLocalModelDownloadProgress.value = {
      ...args.dictationLocalModelDownloadProgress.value,
      [modelId]: {
        phase: "model",
        downloadedBytes: 0,
        totalBytes: null,
        percent: null,
      },
    };
    try {
      const resp = (await window.kepler.ark.request("dictation.download_local_model", {
        modelId,
        select: false,
      })) as {
        config?: Partial<DictationConfigData> | null;
        localModels?: DictationLocalModelsSnapshot;
      };
      applyDictationConfigSnapshot(
        resp.config ?? null,
        args.dictationConfig,
        args.dictationCustomDohUrl,
      );
      if (resp.localModels) args.dictationLocalModels.value = resp.localModels;
    } catch (e) {
      args.dictationLocalModelsError.value = (e as Error).message;
    } finally {
      args.dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationUseLocalModel(modelId: string) {
    args.dictationLocalModelsBusy.value = modelId;
    args.dictationLocalModelsError.value = "";
    try {
      const resp = (await window.kepler.ark.request("dictation.use_local_model", {
        modelId,
      })) as {
        config?: Partial<DictationConfigData>;
        localModels?: DictationLocalModelsSnapshot;
      };
      applyDictationConfigSnapshot(
        resp.config ?? null,
        args.dictationConfig,
        args.dictationCustomDohUrl,
      );
      if (resp.localModels) args.dictationLocalModels.value = resp.localModels;
    } catch (e) {
      args.dictationLocalModelsError.value = (e as Error).message;
    } finally {
      args.dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationDeleteLocalModel(modelId: string) {
    args.dictationLocalModelsBusy.value = modelId;
    args.dictationLocalModelsError.value = "";
    try {
      const resp = (await window.kepler.ark.request("dictation.delete_local_model", {
        modelId,
      })) as {
        config?: Partial<DictationConfigData>;
        localModels?: DictationLocalModelsSnapshot;
      };
      applyDictationConfigSnapshot(
        resp.config ?? null,
        args.dictationConfig,
        args.dictationCustomDohUrl,
      );
      if (resp.localModels) args.dictationLocalModels.value = resp.localModels;
    } catch (e) {
      args.dictationLocalModelsError.value = (e as Error).message;
    } finally {
      args.dictationLocalModelsBusy.value = null;
    }
  }

  const keyActions = createDictationKeyActions({
    ...args,
    patchDictationConfig,
    loadDictationConfig,
  });

  return {
    loadDictationConfig,
    loadDictationLocalModels,
    loadDictationStats,
    loadDictationMicrophones,
    onDictationMicChange,
    onDictationLanguageChange,
    onDictationInjectModeChange,
    onDictationTriggerModeChange,
    onDictationDuckAudioChange,
    onDictationProviderChange,
    onDictationVoiceModelChange,
    onDictationProviderEnabledChange,
    onDictationModelBlur,
    onDictationDownloadLocalModel,
    onDictationUseLocalModel,
    onDictationDeleteLocalModel,
    ...keyActions,
  };
}
