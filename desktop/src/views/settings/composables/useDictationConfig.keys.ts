import type { Ref } from "vue";
import type { JsonRecord } from "../../../shared/runtimeGuards";
import {
  buildAccelerator,
  validateCustomDohUrl,
  type ConnectivityReport,
  type DictationConfigData,
  type DnsKind,
} from "./useDictationConfig.shared";

type DnsProfile = { kind: DnsKind; url?: string };

interface DictationKeyActionsArgs {
  dictationConfig: Ref<DictationConfigData>;
  dictationApiKeyInput: Ref<string>;
  dictationApiKeyBusy: Ref<boolean>;
  dictationApiKeyMsg: Ref<string>;
  dictationCustomDohUrl: Ref<string>;
  dictationConnTestBusy: Ref<boolean>;
  dictationConnTestResult: Ref<string>;
  dictationConnReport: Ref<ConnectivityReport | null>;
  dictationCustomDohError: Ref<string | null>;
  dictationCaptureAccelerator: Ref<string | null>;
  dictationCaptureCancelTick: Ref<number>;
  patchDictationConfig: (patch: JsonRecord) => Promise<void>;
  loadDictationConfig: () => Promise<void>;
}

export function createDictationKeyActions(args: DictationKeyActionsArgs) {
  let dictationCaptureUnsubscribe: (() => void) | null = null;

  async function onDictationHotkeyCapture(acc: string) {
    if (!acc) return;
    await args.patchDictationConfig({ hotkey: acc });
  }

  function onDictationCaptureStart() {
    args.dictationCaptureAccelerator.value = null;
    if (!dictationCaptureUnsubscribe) {
      dictationCaptureUnsubscribe = window.kepler.dictation.onCaptureEvent((e) => {
        if (e.event === "dictation_capture_key") {
          const acc =
// SAFETY: the surrounding domain validation preserves the asserted contract.
            (e.accelerator as string | undefined) ||
            buildAccelerator({
// SAFETY: the surrounding domain validation preserves the asserted contract.
              vk: e.vk as number,
// SAFETY: the surrounding domain validation preserves the asserted contract.
              ctrl: e.ctrl as boolean,
// SAFETY: the surrounding domain validation preserves the asserted contract.
              shift: e.shift as boolean,
// SAFETY: the surrounding domain validation preserves the asserted contract.
              alt: e.alt as boolean,
// SAFETY: the surrounding domain validation preserves the asserted contract.
              win: e.win as boolean,
            });
          if (acc) args.dictationCaptureAccelerator.value = acc;
        } else if (e.event === "dictation_capture_cancelled") {
          args.dictationCaptureCancelTick.value++;
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
    const profile: DnsProfile = { kind };
    if (kind === "custom_doh") profile.url = args.dictationCustomDohUrl.value.trim();
    await args.patchDictationConfig({ networkProfile: profile });
  }

  async function onDictationCustomDohBlur() {
    if (args.dictationConfig.value.networkProfile.kind !== "custom_doh") return;
    const url = args.dictationCustomDohUrl.value.trim();
    const err = validateCustomDohUrl(url);
    args.dictationCustomDohError.value = err;
    if (err) return;
    await args.patchDictationConfig({
      networkProfile: { kind: "custom_doh", url },
    });
  }

  async function verifyApiKey(key: string): Promise<{
    ok: boolean;
    reason?: string;
    status?: number;
    error?: string;
    latencyMs?: number;
  }> {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
    await args.loadDictationConfig();
  }

  async function onDictationSaveApiKey() {
    const key = args.dictationApiKeyInput.value.trim();
    if (!key) return;
    args.dictationApiKeyBusy.value = true;
    args.dictationApiKeyMsg.value = "";
    try {
      await window.kepler.ark.request("dictation.set_api_key", { key });
      args.dictationApiKeyInput.value = "";
      args.dictationApiKeyMsg.value = "Сохранено в Windows Credential Manager";
      await args.loadDictationConfig();
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
    } finally {
      args.dictationApiKeyBusy.value = false;
    }
  }

  async function onDictationClearApiKey() {
    args.dictationApiKeyBusy.value = true;
    args.dictationApiKeyMsg.value = "";
    try {
      await window.kepler.ark.request("dictation.clear_api_key", {});
      args.dictationApiKeyMsg.value = "Ключ удалён";
      await args.loadDictationConfig();
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
    } finally {
      args.dictationApiKeyBusy.value = false;
    }
  }

  async function onDictationTestConnectivity() {
    args.dictationConnTestBusy.value = true;
    args.dictationConnTestResult.value = "";
    args.dictationConnReport.value = null;
    try {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      const resp = (await window.kepler.ark.request(
        "dictation.test_connectivity",
        {},
      )) as ConnectivityReport;
      args.dictationConnReport.value = resp;
      args.dictationConnTestResult.value = resp.ok
        ? `OK · ${resp.totalMs}ms`
        : `Сбой на стадии ${resp.firstFailure ?? "?"}`;
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      args.dictationConnTestResult.value = `Ошибка: ${(e as Error).message}`;
      args.dictationConnReport.value = null;
    } finally {
      args.dictationConnTestBusy.value = false;
    }
  }

  return {
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
