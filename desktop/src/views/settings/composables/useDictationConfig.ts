// useDictationConfig — единый state + handlers для диктации и AI settings.
// Используется несколькими tab'ами SettingsView'а, поэтому composable должен
// быть SINGLETON на уровне owner'а — иначе копии state разойдутся. Реализуем
// через `provide`/`inject` (DictationConfigKey).

import { computed, type InjectionKey, ref } from "vue";

export type DnsKind = "system" | "cloudflare_doh" | "google_doh" | "custom_doh";
export type DictationProvider = "groq" | "mock" | "local";

export interface DictationConfigData {
  hotkey: string;
  triggerMode: "toggle" | "push_to_talk";
  language: string;
  injectMode: "auto_paste" | "clipboard_only";
  networkProfile: { kind: DnsKind; url?: string };
  httpProxy: string | null;
  transcriptionPrompt: string;
  provider: DictationProvider;
  providerEnabled: boolean;
  model: string;
  localModelPath: string | null;
  localCommandPath: string | null;
  localModelId: string;
  localEngine: string;
  microphoneDeviceId: string | null;
}

/// Стадия probe в `dictation.test_connectivity`. См. backend
/// `platform/runtime/src/dictation/host.rs::probe_connectivity`.
export interface ConnectivityStage {
  name: "client_build" | "dns_resolve" | "tcp_connect" | "http_head";
  ok: boolean;
  ms: number;
  error?: string;
  ip?: string;
  status?: number;
}

export interface ConnectivityReport {
  ok: boolean;
  totalMs: number;
  stages: ConnectivityStage[];
  firstFailure: string | null;
}

export interface DictationStatsData {
  totalWords: number;
  totalRecordSeconds: number;
  totalSessions: number;
  wpm: number;
  timeSavedSeconds: number;
}

export interface DictationLocalModelInfo {
  id: string;
  name: string;
  description: string;
  filename: string;
  url: string;
  sizeMb: number;
  accuracyScore: number;
  speedScore: number;
  recommended: boolean;
  downloaded: boolean;
  selected: boolean;
  path: string | null;
}

export interface DictationLocalModelsSnapshot {
  modelsDir: string;
  commandPath: string | null;
  commandInstalled: boolean;
  models: DictationLocalModelInfo[];
}

export interface DictationVoiceModelOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

const DEFAULT_DICTATION_CFG: DictationConfigData = {
  hotkey: "Ctrl+Shift+;",
  triggerMode: "toggle",
  language: "ru",
  injectMode: "auto_paste",
  networkProfile: { kind: "system" },
  httpProxy: null,
  transcriptionPrompt: "",
  provider: "groq",
  providerEnabled: true,
  model: "whisper-large-v3",
  localModelPath: null,
  localCommandPath: null,
  localModelId: "whisper-large-v3",
  localEngine: "whisper.cpp",
  microphoneDeviceId: null,
};

const DEFAULT_DICTATION_STATS: DictationStatsData = {
  totalWords: 0,
  totalRecordSeconds: 0,
  totalSessions: 0,
  wpm: 0,
  timeSavedSeconds: 0,
};

export const DNS_PROFILE_OPTIONS = [
  { value: "system", label: "Системный" },
  { value: "cloudflare_doh", label: "Cloudflare (1.1.1.1)" },
  { value: "google_doh", label: "Google (8.8.8.8)" },
  { value: "custom_doh", label: "Свой DoH URL" },
] as const;

export const DICTATION_TRIGGER_OPTIONS = [
  {
    value: "toggle",
    label: "Toggle (двойное нажатие)",
    description: "Первое нажатие — старт, второе — отправка.",
  },
  {
    value: "push_to_talk",
    label: "Push-to-talk (удерживать)",
    description: "Удерживайте клавишу пока говорите.",
  },
] as const;

export const DICTATION_INJECT_OPTIONS = [
  {
    value: "auto_paste",
    label: "Auto-paste",
    description: "Симулирует Ctrl+V и восстанавливает буфер.",
  },
  {
    value: "clipboard_only",
    label: "Только в буфер обмена",
    description: "Текст записывается в буфер, Ctrl+V — вручную.",
  },
] as const;

export const DICTATION_LANGUAGE_OPTIONS = [
  { value: "auto", label: "Авто" },
  { value: "ru", label: "Русский" },
  { value: "en", label: "English" },
  { value: "uk", label: "Українська" },
  { value: "be", label: "Беларуская" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
  { value: "it", label: "Italiano" },
  { value: "pt", label: "Português" },
  { value: "pl", label: "Polski" },
  { value: "tr", label: "Türkçe" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "zh", label: "中文" },
  { value: "ar", label: "العربية" },
  { value: "he", label: "עברית" },
  { value: "hi", label: "हिन्दी" },
  { value: "nl", label: "Nederlands" },
  { value: "sv", label: "Svenska" },
  { value: "fi", label: "Suomi" },
  { value: "cs", label: "Čeština" },
  { value: "el", label: "Ελληνικά" },
] as const;

const DICTATION_GROQ_SPEECH_MODELS = [
  { id: "whisper-large-v3-turbo", label: "Groq · Whisper Large V3 Turbo" },
  { id: "whisper-large-v3", label: "Groq · Whisper Large V3" },
] as const;

function vkToKeyName(vk: number): string {
  if ((vk >= 0x41 && vk <= 0x5a) || (vk >= 0x30 && vk <= 0x39)) {
    return String.fromCharCode(vk);
  }
  if (vk >= 0x70 && vk <= 0x87) return `F${vk - 0x6f}`;
  const oem: Record<number, string> = {
    0xba: ";",
    0xbb: "+",
    0xbc: ",",
    0xbd: "-",
    0xbe: ".",
    0xbf: "/",
    0xc0: "`",
    0xdb: "[",
    0xdc: "\\",
    0xdd: "]",
    0xde: "'",
  };
  if (oem[vk]) return oem[vk];
  const named: Record<number, string> = {
    0x08: "Backspace",
    0x09: "Tab",
    0x0d: "Enter",
    0x20: "Space",
    0x21: "PageUp",
    0x22: "PageDown",
    0x23: "End",
    0x24: "Home",
    0x25: "Left",
    0x26: "Up",
    0x27: "Right",
    0x28: "Down",
    0x2d: "Insert",
    0x2e: "Delete",
  };
  return named[vk] ?? "";
}

function buildAccelerator(payload: {
  vk: number;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  win?: boolean;
}): string {
  const parts: string[] = [];
  if (payload.ctrl) parts.push("Ctrl");
  if (payload.alt) parts.push("Alt");
  if (payload.shift) parts.push("Shift");
  if (payload.win) parts.push("Super");
  const key = vkToKeyName(payload.vk);
  if (!key) return "";
  parts.push(key);
  return parts.join("+");
}

function formatTimeSaved(sec: number): { value: string; unit: string } {
  if (sec < 60) return { value: String(Math.max(0, Math.round(sec))), unit: "сек" };
  if (sec < 3600) return { value: String(Math.round(sec / 60)), unit: "мин" };
  return { value: (sec / 3600).toFixed(1), unit: "ч" };
}

function formatTotalWords(n: number): { value: string; unit: string } {
  if (n < 1000) return { value: String(n), unit: "" };
  if (n < 1_000_000) return { value: (n / 1000).toFixed(1), unit: "k" };
  return { value: (n / 1_000_000).toFixed(1), unit: "M" };
}

export function createDictationConfig() {
  const dictationConfig = ref<DictationConfigData>({ ...DEFAULT_DICTATION_CFG });
  const dictationStats = ref<DictationStatsData>({ ...DEFAULT_DICTATION_STATS });
  const dictationHasApiKey = ref(false);
  const dictationApiKeyInput = ref("");
  const dictationApiKeyBusy = ref(false);
  const dictationApiKeyMsg = ref("");
  const dictationCustomDohUrl = ref("");
  const dictationConnTestBusy = ref(false);
  const dictationConnTestResult = ref<string>("");
  // Структурированный отчёт от dictation.test_connectivity (Phase 2).
  // `null` пока не запускали; обновляется на каждый клик «Проверить».
  const dictationConnReport = ref<ConnectivityReport | null>(null);
  // Валидация Custom DoH URL — frontend-сторона. На @blur устанавливается
  // в Some(error) если URL невалиден; backend дублирует, но frontend быстрее.
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
  let dictationCaptureUnsubscribe: (() => void) | null = null;
  let dictationArkUnsubscribe: (() => void) | null = null;

  function ensureDictationEventSubscription() {
    if (dictationArkUnsubscribe) return;
    dictationArkUnsubscribe = window.kepler.ark.onEvent((event) => {
      const kind = event.event;
      const modelId = typeof event.modelId === "string" ? event.modelId : "";
      if (!modelId) return;
      if (kind === "dictation_local_model_download_started") {
        dictationLocalModelsError.value = "";
        dictationLocalModelDownloadProgress.value = {
          ...dictationLocalModelDownloadProgress.value,
          [modelId]: {
            phase: "model",
            downloadedBytes: 0,
            totalBytes: null,
            percent: null,
          },
        };
      } else if (kind === "dictation_local_model_download_progress") {
        dictationLocalModelDownloadProgress.value = {
          ...dictationLocalModelDownloadProgress.value,
          [modelId]: {
            phase: typeof event.phase === "string" ? event.phase : "download",
            downloadedBytes: typeof event.downloadedBytes === "number" ? event.downloadedBytes : 0,
            totalBytes: typeof event.totalBytes === "number" ? event.totalBytes : null,
            percent: typeof event.percent === "number" ? event.percent : null,
          },
        };
      } else if (kind === "dictation_local_model_download_complete") {
        if (event.config && typeof event.config === "object") {
          const config = event.config as Partial<DictationConfigData>;
          dictationConfig.value = {
            ...DEFAULT_DICTATION_CFG,
            ...config,
            networkProfile: config.networkProfile ?? { kind: "system" },
            localModelPath: config.localModelPath ?? null,
            localCommandPath: config.localCommandPath ?? null,
            localModelId: config.localModelId ?? DEFAULT_DICTATION_CFG.localModelId,
            localEngine: config.localEngine ?? DEFAULT_DICTATION_CFG.localEngine,
          };
        }
        if (event.localModels && typeof event.localModels === "object") {
          dictationLocalModels.value = event.localModels as DictationLocalModelsSnapshot;
        } else {
          void loadDictationLocalModels();
        }
        const next = { ...dictationLocalModelDownloadProgress.value };
        delete next[modelId];
        dictationLocalModelDownloadProgress.value = next;
      } else if (kind === "dictation_local_model_download_failed") {
        const next = { ...dictationLocalModelDownloadProgress.value };
        delete next[modelId];
        dictationLocalModelDownloadProgress.value = next;
        dictationLocalModelsError.value =
          typeof event.error === "string" ? event.error : "Скачивание не удалось.";
      }
    });
  }

  const dictationMicOptions = computed(() => {
    const opts: { value: string; label: string }[] = [
      { value: "default", label: "Системный по умолчанию" },
    ];
    for (const d of dictationMicDevices.value) {
      opts.push({ value: d.deviceId, label: d.label });
    }
    return opts;
  });

  const statsCards = computed(() => {
    const wpm = Math.round(dictationStats.value.wpm);
    const saved = formatTimeSaved(dictationStats.value.timeSavedSeconds);
    const total = formatTotalWords(dictationStats.value.totalWords);
    return [
      { key: "wpm", label: "WPM", value: String(wpm), unit: "" },
      { key: "saved", label: "Сэкономлено", value: saved.value, unit: saved.unit },
      { key: "words", label: "Всего слов", value: total.value, unit: total.unit },
    ];
  });

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

  const dictationLocalCommandPath = computed<string>({
    get: () => dictationConfig.value.localCommandPath ?? "",
    set: (value) => {
      dictationConfig.value.localCommandPath = value;
    },
  });

  const dictationLocalModelId = computed<string>({
    get: () => dictationConfig.value.localModelId,
    set: (value) => {
      dictationConfig.value.localModelId = value;
    },
  });

  const dictationLocalEngine = computed<string>({
    get: () => dictationConfig.value.localEngine,
    set: (value) => {
      dictationConfig.value.localEngine = value;
    },
  });

  const dictationVoiceModelOptions = computed<DictationVoiceModelOption[]>(() => {
    const options: DictationVoiceModelOption[] = [];

    if (dictationHasApiKey.value) {
      for (const model of DICTATION_GROQ_SPEECH_MODELS) {
        options.push({
          value: `groq:${model.id}`,
          label: model.label,
          description: "Онлайн-распознавание речи через Groq",
        });
      }
    }

    for (const model of dictationLocalModels.value?.models ?? []) {
      if (!model.downloaded && !model.selected) continue;
      options.push({
        value: `local:${model.id}`,
        label: `OpenAI · ${model.name}`,
        description: "Локальное распознавание речи",
      });
    }

    return options;
  });

  const dictationVoiceModelValue = computed(() => {
    if (dictationConfig.value.provider === "groq") {
      return `groq:${dictationConfig.value.model || DEFAULT_DICTATION_CFG.model}`;
    }
    if (dictationConfig.value.provider === "local") {
      return `local:${dictationConfig.value.localModelId || DEFAULT_DICTATION_CFG.localModelId}`;
    }
    return "";
  });

  const dictationGroqStatus = computed(() => {
    if (dictationConfig.value.provider !== "groq") {
      return dictationHasApiKey.value
        ? "Groq-ключ сохранён, но сейчас выбрана локальная модель."
        : "Groq-ключ не задан.";
    }
    return dictationHasApiKey.value
      ? "Groq-ключ установлен."
      : "Groq-ключ не задан — добавьте его в разделе «AI».";
  });

  const dictationLocalStatus = computed(() => {
    const path = dictationLocalModelPath.value.trim();
    const commandPath = dictationLocalCommandPath.value.trim();
    const normalizedEngine = dictationLocalEngine.value.trim().toLowerCase();
    const needsWhisperCppCommand =
      normalizedEngine !== "faster-whisper" && normalizedEngine !== "faster_whisper";
    if (dictationConfig.value.provider !== "local") {
      return path && (!needsWhisperCppCommand || commandPath)
        ? `Локальная модель подготовлена: ${dictationLocalEngine.value} · ${dictationLocalModelId.value}`
        : "Локальный режим ещё не настроен.";
    }
    if (!path) return "Не задан путь к локальной модели.";
    if (needsWhisperCppCommand && !commandPath) {
      return "Не задан путь к whisper.cpp executable.";
    }
    return `${dictationLocalEngine.value} · ${dictationLocalModelId.value} · путь задан`;
  });

  async function loadDictationLocalModels() {
    dictationLocalModelsError.value = "";
    try {
      dictationLocalModels.value = (await window.kepler.ark.request(
        "dictation.list_local_models",
        {},
      )) as DictationLocalModelsSnapshot;
    } catch (e) {
      dictationLocalModelsError.value = (e as Error).message;
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
        dictationConfig.value = {
          ...DEFAULT_DICTATION_CFG,
          ...resp.config,
          networkProfile: resp.config.networkProfile ?? { kind: "system" },
          localModelPath: resp.config.localModelPath ?? null,
          localCommandPath: resp.config.localCommandPath ?? null,
          localModelId: resp.config.localModelId ?? DEFAULT_DICTATION_CFG.localModelId,
          localEngine: resp.config.localEngine ?? DEFAULT_DICTATION_CFG.localEngine,
        };
        if (dictationConfig.value.networkProfile.kind === "custom_doh") {
          dictationCustomDohUrl.value = dictationConfig.value.networkProfile.url ?? "";
        }
      }
      dictationHasApiKey.value = resp.hasApiKey ?? false;
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
      dictationStats.value = { ...DEFAULT_DICTATION_STATS, ...resp };
    } catch (e) {
      console.error("[settings] loadDictationStats failed:", e);
    }
  }

  async function loadDictationMicrophones() {
    dictationMicError.value = "";
    try {
      try {
        const probe = await navigator.mediaDevices.getUserMedia({ audio: true });
        for (const t of probe.getTracks()) t.stop();
      } catch (permErr) {
        dictationMicError.value =
          "Нет доступа к микрофону — выберите устройство после первого запуска записи.";
        console.warn("[settings] mic permission probe failed:", permErr);
      }
      const devices = await navigator.mediaDevices.enumerateDevices();
      dictationMicDevices.value = devices
        .filter((d) => d.kind === "audioinput")
        .map((d, i) => ({
          deviceId: d.deviceId,
          label: d.label || `Микрофон ${i + 1}`,
        }));
    } catch (e) {
      console.error("[settings] loadDictationMicrophones failed:", e);
      dictationMicError.value = `Ошибка: ${(e as Error).message}`;
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
    const model = dictationModelName.value.trim();
    await patchDictationConfig({ model: model || DEFAULT_DICTATION_CFG.model });
  }

  async function onDictationDownloadLocalModel(modelId: string) {
    dictationLocalModelsBusy.value = modelId;
    dictationLocalModelsError.value = "";
    dictationLocalModelDownloadProgress.value = {
      ...dictationLocalModelDownloadProgress.value,
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
        installTool: true,
      })) as {
        config?: Partial<DictationConfigData> | null;
        localModels?: DictationLocalModelsSnapshot;
      };
      if (resp.config) {
        dictationConfig.value = {
          ...DEFAULT_DICTATION_CFG,
          ...resp.config,
          networkProfile: resp.config.networkProfile ?? { kind: "system" },
          localModelPath: resp.config.localModelPath ?? null,
          localCommandPath: resp.config.localCommandPath ?? null,
          localModelId: resp.config.localModelId ?? DEFAULT_DICTATION_CFG.localModelId,
          localEngine: resp.config.localEngine ?? DEFAULT_DICTATION_CFG.localEngine,
        };
      }
      if (resp.localModels) dictationLocalModels.value = resp.localModels;
    } catch (e) {
      dictationLocalModelsError.value = (e as Error).message;
    } finally {
      dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationUseLocalModel(modelId: string) {
    dictationLocalModelsBusy.value = modelId;
    dictationLocalModelsError.value = "";
    try {
      const resp = (await window.kepler.ark.request("dictation.use_local_model", {
        modelId,
      })) as {
        config?: Partial<DictationConfigData>;
        localModels?: DictationLocalModelsSnapshot;
      };
      if (resp.config) {
        dictationConfig.value = {
          ...DEFAULT_DICTATION_CFG,
          ...resp.config,
          networkProfile: resp.config.networkProfile ?? { kind: "system" },
          localModelPath: resp.config.localModelPath ?? null,
          localCommandPath: resp.config.localCommandPath ?? null,
          localModelId: resp.config.localModelId ?? DEFAULT_DICTATION_CFG.localModelId,
          localEngine: resp.config.localEngine ?? DEFAULT_DICTATION_CFG.localEngine,
        };
      }
      if (resp.localModels) dictationLocalModels.value = resp.localModels;
    } catch (e) {
      dictationLocalModelsError.value = (e as Error).message;
    } finally {
      dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationDeleteLocalModel(modelId: string) {
    dictationLocalModelsBusy.value = modelId;
    dictationLocalModelsError.value = "";
    try {
      const resp = (await window.kepler.ark.request("dictation.delete_local_model", {
        modelId,
      })) as {
        config?: Partial<DictationConfigData>;
        localModels?: DictationLocalModelsSnapshot;
      };
      if (resp.config) {
        dictationConfig.value = {
          ...DEFAULT_DICTATION_CFG,
          ...resp.config,
          networkProfile: resp.config.networkProfile ?? { kind: "system" },
          localModelPath: resp.config.localModelPath ?? null,
          localCommandPath: resp.config.localCommandPath ?? null,
          localModelId: resp.config.localModelId ?? DEFAULT_DICTATION_CFG.localModelId,
          localEngine: resp.config.localEngine ?? DEFAULT_DICTATION_CFG.localEngine,
        };
      }
      if (resp.localModels) dictationLocalModels.value = resp.localModels;
    } catch (e) {
      dictationLocalModelsError.value = (e as Error).message;
    } finally {
      dictationLocalModelsBusy.value = null;
    }
  }

  async function onDictationHotkeyCapture(acc: string) {
    if (!acc) return;
    await patchDictationConfig({ hotkey: acc });
  }

  function onDictationCaptureStart() {
    dictationCaptureAccelerator.value = null;
    if (!dictationCaptureUnsubscribe) {
      dictationCaptureUnsubscribe = window.kepler.dictation.onCaptureEvent((e) => {
        if (e.event === "dictation_capture_key") {
          // Backend может прислать готовый accelerator (macOS-адаптер резолвит
          // mac keyCode в строку сам) — тогда используем его. Иначе собираем из
          // Windows-VK. Ветка платформо-агностична: «если резолвнуто — берём».
          const acc =
            (e.accelerator as string | undefined) ||
            buildAccelerator({
              vk: e.vk as number,
              ctrl: e.ctrl as boolean,
              shift: e.shift as boolean,
              alt: e.alt as boolean,
              win: e.win as boolean,
            });
          if (acc) dictationCaptureAccelerator.value = acc;
        } else if (e.event === "dictation_capture_cancelled") {
          dictationCaptureCancelTick.value++;
        }
      });
    }
    void window.kepler.ark.request("dictation.begin_hotkey_capture", {});
  }

  function onDictationCaptureEnd() {
    void window.kepler.ark.request("dictation.end_hotkey_capture", {});
    dictationCaptureUnsubscribe?.();
    dictationCaptureUnsubscribe = null;
  }

  async function onDictationDnsKindChange(kind: DnsKind) {
    const profile: { kind: DnsKind; url?: string } = { kind };
    if (kind === "custom_doh") profile.url = dictationCustomDohUrl.value.trim();
    await patchDictationConfig({ networkProfile: profile });
  }

  /// Чистая валидация — синхронная, без backend round-trip.
  /// Те же правила что в `network::validate_custom_doh_url` на backend.
  function validateCustomDohUrl(raw: string): string | null {
    const url = raw.trim();
    if (!url) return "URL пустой";
    if (!url.startsWith("https://")) return "URL должен начинаться с https://";
    const rest = url.slice("https://".length);
    if (!rest) return "Хост не задан";
    if (rest.includes("@")) return "userinfo (user:pass@host) не поддерживается";
    const slash = rest.indexOf("/");
    const authority = slash >= 0 ? rest.slice(0, slash) : rest;
    if (!authority) return "Хост не задан";
    return null;
  }

  async function onDictationCustomDohBlur() {
    if (dictationConfig.value.networkProfile.kind !== "custom_doh") return;
    const url = dictationCustomDohUrl.value.trim();
    const err = validateCustomDohUrl(url);
    dictationCustomDohError.value = err;
    if (err) return; // невалидный URL — не отправляем patch
    await patchDictationConfig({
      networkProfile: { kind: "custom_doh", url },
    });
  }

  /// Лёгкая проверка API key без сохранения — GET /v1/models с Bearer-auth.
  /// Возвращает структуру `{ ok, reason?, status?, error?, latencyMs? }`.
  /// `reason`: empty_key | invalid_key | provider_error | network | client_build.
  async function verifyApiKey(key: string): Promise<{
    ok: boolean;
    reason?: string;
    status?: number;
    error?: string;
    latencyMs?: number;
  }> {
    return (await window.kepler.ark.request("dictation.verify_api_key", { key })) as {
      ok: boolean;
      reason?: string;
      status?: number;
      error?: string;
      latencyMs?: number;
    };
  }

  async function saveApiKey(key: string): Promise<void> {
    await window.kepler.ark.request("dictation.set_api_key", { key });
    await loadDictationConfig();
  }

  async function onDictationSaveApiKey() {
    const key = dictationApiKeyInput.value.trim();
    if (!key) return;
    dictationApiKeyBusy.value = true;
    dictationApiKeyMsg.value = "";
    try {
      await window.kepler.ark.request("dictation.set_api_key", { key });
      dictationApiKeyInput.value = "";
      dictationApiKeyMsg.value = "Сохранено в Windows Credential Manager";
      await loadDictationConfig();
    } catch (e) {
      dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
    } finally {
      dictationApiKeyBusy.value = false;
    }
  }

  async function onDictationClearApiKey() {
    dictationApiKeyBusy.value = true;
    dictationApiKeyMsg.value = "";
    try {
      await window.kepler.ark.request("dictation.clear_api_key", {});
      dictationApiKeyMsg.value = "Ключ удалён";
      await loadDictationConfig();
    } catch (e) {
      dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
    } finally {
      dictationApiKeyBusy.value = false;
    }
  }

  async function onDictationTestConnectivity() {
    dictationConnTestBusy.value = true;
    dictationConnTestResult.value = "";
    dictationConnReport.value = null;
    try {
      const resp = (await window.kepler.ark.request(
        "dictation.test_connectivity",
        {},
      )) as ConnectivityReport;
      dictationConnReport.value = resp;
      if (resp.ok) {
        dictationConnTestResult.value = `OK · ${resp.totalMs}ms`;
      } else {
        dictationConnTestResult.value = `Сбой на стадии ${resp.firstFailure ?? "?"}`;
      }
    } catch (e) {
      dictationConnTestResult.value = `Ошибка: ${(e as Error).message}`;
      dictationConnReport.value = null;
    } finally {
      dictationConnTestBusy.value = false;
    }
  }

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
    dictationMicOptions,
    statsCards,
    dictationGroqStatus,
    dictationLocalStatus,
    loadDictationConfig,
    loadDictationLocalModels,
    loadDictationStats,
    loadDictationMicrophones,
    onDictationMicChange,
    onDictationLanguageChange,
    onDictationInjectModeChange,
    onDictationTriggerModeChange,
    onDictationVoiceModelChange,
    onDictationProviderEnabledChange,
    onDictationModelBlur,
    onDictationDownloadLocalModel,
    onDictationUseLocalModel,
    onDictationDeleteLocalModel,
    onDictationHotkeyCapture,
    onDictationCaptureStart,
    onDictationCaptureEnd,
    onDictationDnsKindChange,
    onDictationCustomDohBlur,
    onDictationSaveApiKey,
    onDictationClearApiKey,
    verifyApiKey,
    saveApiKey,
    onDictationTestConnectivity,
  };
}

export type DictationConfigCtx = ReturnType<typeof createDictationConfig>;

export const DictationConfigKey: InjectionKey<DictationConfigCtx> = Symbol("DictationConfigKey");
