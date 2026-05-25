// useDictationConfig — единый state + handlers для диктации (Phase 1, Groq).
// Используется тремя tab'ами: Dictation, Security (DNS profile + proxy),
// Secrets (API key). Поэтому composable должен быть SINGLETON на уровне owner'а
// SettingsView'а — иначе три копии state разойдутся. Реализуем через
// `provide`/`inject` (DictationConfigKey).

import { computed, type InjectionKey, ref } from "vue";

export type DnsKind = "system" | "cloudflare_doh" | "google_doh" | "custom_doh";

export interface DictationConfigData {
  hotkey: string;
  triggerMode: "toggle" | "push_to_talk";
  language: string;
  injectMode: "auto_paste" | "clipboard_only";
  networkProfile: { kind: DnsKind; url?: string };
  httpProxy: string | null;
  transcriptionPrompt: string;
  provider: string;
  model: string;
  microphoneDeviceId: string | null;
}

export interface DictationStatsData {
  totalWords: number;
  totalRecordSeconds: number;
  totalSessions: number;
  wpm: number;
  timeSavedSeconds: number;
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
  model: "whisper-large-v3-turbo",
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
  {
    value: "system",
    label: "Системный DNS",
    description: "OS resolver. Самый совместимый, но в РФ Groq обычно блокирован.",
  },
  {
    value: "cloudflare_doh",
    label: "Cloudflare DoH (1.1.1.1)",
    description: "Обходит DNS poisoning. Безопасный default.",
  },
  { value: "google_doh", label: "Google DoH (8.8.8.8)", description: "Альтернатива Cloudflare." },
  {
    value: "custom_doh",
    label: "Свой DoH URL",
    description: "Укажите endpoint в формате https://example/dns-query.",
  },
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

export const DICTATION_PROVIDER_OPTIONS = [{ value: "groq", label: "Groq Cloud" }] as const;

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
  const dictationProxyInput = ref("");
  const dictationConnTestBusy = ref(false);
  const dictationConnTestResult = ref<string>("");
  const dictationMicDevices = ref<{ deviceId: string; label: string }[]>([]);
  const dictationMicError = ref<string>("");

  const dictationCaptureAccelerator = ref<string | null>(null);
  const dictationCaptureCancelTick = ref(0);
  let dictationCaptureUnsubscribe: (() => void) | null = null;

  const dictationProviderDescription = computed(() =>
    dictationHasApiKey.value
      ? "Ключ установлен в разделе «Секреты»."
      : "Не задан API-ключ — добавьте его в разделе «Секреты».",
  );

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

  async function loadDictationConfig() {
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
        };
        if (dictationConfig.value.networkProfile.kind === "custom_doh") {
          dictationCustomDohUrl.value = dictationConfig.value.networkProfile.url ?? "";
        }
        dictationProxyInput.value = dictationConfig.value.httpProxy ?? "";
      }
      dictationHasApiKey.value = resp.hasApiKey ?? false;
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

  async function onDictationProviderChange(v: string) {
    await patchDictationConfig({ provider: v });
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
          const acc = buildAccelerator({
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

  async function onDictationCustomDohBlur() {
    if (dictationConfig.value.networkProfile.kind !== "custom_doh") return;
    await patchDictationConfig({
      networkProfile: { kind: "custom_doh", url: dictationCustomDohUrl.value.trim() },
    });
  }

  async function onDictationProxyBlur() {
    const v = dictationProxyInput.value.trim();
    await patchDictationConfig({ httpProxy: v === "" ? null : v });
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
    try {
      const resp = (await window.kepler.ark.request("dictation.test_connectivity", {})) as {
        ok?: boolean;
        status?: number;
        latencyMs?: number;
      };
      if (resp.ok) {
        dictationConnTestResult.value = `OK · ${resp.status ?? "—"} · ${resp.latencyMs ?? "?"}ms`;
      } else {
        dictationConnTestResult.value = "Не удалось";
      }
    } catch (e) {
      dictationConnTestResult.value = `Ошибка: ${(e as Error).message}`;
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
    dictationProxyInput,
    dictationConnTestBusy,
    dictationConnTestResult,
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
    onDictationDnsKindChange,
    onDictationCustomDohBlur,
    onDictationProxyBlur,
    onDictationSaveApiKey,
    onDictationClearApiKey,
    onDictationTestConnectivity,
  };
}

export type DictationConfigCtx = ReturnType<typeof createDictationConfig>;

export const DictationConfigKey: InjectionKey<DictationConfigCtx> = Symbol("DictationConfigKey");
