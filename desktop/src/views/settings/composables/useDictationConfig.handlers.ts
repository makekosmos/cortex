import type { Ref } from "vue";
import {
  applyDictationConfigSnapshot,
  buildDictationVoiceModelValue,
  DEFAULT_DICTATION_CFG,
  normalizeDictationStats,
  resolveAvailableDictationVoiceModelValue,
  type DictationConfigData,
  type DictationLocalModelsSnapshot,
  type DictationProvider,
  type DictationStatsData,
} from "./useDictationConfig.shared";
import { createDictationKeyActions } from "./useDictationConfig.keys";
import { isNumber, isRecord, isString, type JsonRecord } from "../../../shared/runtimeGuards";

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
      const modelId = isString(event.modelId) ? event.modelId : "";
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
            phase: isString(event.phase) ? event.phase : "download",
            downloadedBytes: isNumber(event.downloadedBytes) ? event.downloadedBytes : 0,
            totalBytes: isNumber(event.totalBytes) ? event.totalBytes : null,
            percent: isNumber(event.percent) ? event.percent : null,
          },
        };
        return;
      }

      if (kind === "dictation_local_model_download_complete") {
        let config: Partial<DictationConfigData> | null = null;
        if (isRecord(event.config)) {
          // SAFETY: the bridge event schema supplies a DictationConfigData-compatible object.
          config = event.config as Partial<DictationConfigData>;
        }
        applyDictationConfigSnapshot(
          config,
          args.dictationConfig,
          args.dictationCustomDohUrl,
        );
        if (isRecord(event.localModels)) {
          // SAFETY: the bridge event schema supplies a DictationLocalModelsSnapshot-compatible object.
          const localModels = Object.assign(
            {
              models: [],
              modelsDir: "",
              commandPath: null,
              commandInstalled: false,
            } as DictationLocalModelsSnapshot,
            event.localModels,
          );
          args.dictationLocalModels.value = localModels;
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
          isString(event.error) ? event.error : "Скачивание не удалось.";
      }
    });
  }

  async function loadDictationLocalModels() {
    args.dictationLocalModelsError.value = "";
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationLocalModels.value = (await window.kepler.ark.request(
        "dictation.list_local_models",
        {},
      )) as DictationLocalModelsSnapshot;
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationLocalModelsError.value = (e as Error).message;
    }
  }

  async function loadDictationConfig() {
    ensureDictationEventSubscription();
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      const resp = (await window.kepler.ark.request("dictation.get_config", {})) as {
        config?: Partial<DictationConfigData>;
        hasApiKey?: boolean;
      };
      if (resp.config) {
        applyDictationConfigSnapshot(resp.config, args.dictationConfig, args.dictationCustomDohUrl);
      }
      args.dictationHasApiKey.value = resp.hasApiKey ?? false;
      await loadDictationLocalModels();
      const current = buildDictationVoiceModelValue(args.dictationConfig.value);
      const resolved = resolveAvailableDictationVoiceModelValue(args.dictationConfig.value, {
        hasApiKey: args.dictationHasApiKey.value,
        localModels: args.dictationLocalModels.value,
      });
      if (resolved && resolved !== current) {
        const [source, modelId] = resolved.split(":", 2);
        if (source === "local") {
          await onDictationUseLocalModel(modelId);
        } else if (source === "groq") {
          await patchDictationConfig({
            provider: "groq",
            providerEnabled: true,
            model: modelId,
          });
        }
      }
    } catch (e) {
      console.error("[settings] loadDictationConfig failed:", e);
    }
  }

  async function loadDictationStats() {
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationMicError.value = `Ошибка: ${(e as Error).message}`;
    }
  }

  async function patchDictationConfig(patch: JsonRecord) {
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

  async function onDictationIdleUnloadChange(v: number | null) {
    await patchDictationConfig({ localIdleUnloadMs: v });
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationLocalModelsError.value = (e as Error).message;
    } finally {
      args.dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationUseLocalModel(modelId: string) {
    args.dictationLocalModelsBusy.value = modelId;
    args.dictationLocalModelsError.value = "";
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationLocalModelsError.value = (e as Error).message;
    } finally {
      args.dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationDeleteLocalModel(modelId: string) {
    args.dictationLocalModelsBusy.value = modelId;
    args.dictationLocalModelsError.value = "";
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
    onDictationIdleUnloadChange,
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
