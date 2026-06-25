import { setDictationShortcutResolver } from "./commands";
import {
  setDictationCommandInvoker,
  setDictationRuntime,
  setupDictationHotkey,
} from "./dictation-pill";
import {
  setFocusWidgetFocusSessionOpener,
  setFocusWidgetRuntime,
  setupFocusWidgetBackendSync,
  teardownFocusWidgetBackendSync,
} from "./focus-widget";
import {
  openFocusSessionShell,
  setupFocusSessionBackendSync,
  teardownFocusSessionBackendSync,
} from "./focus-session";
import { setupPomodoroNotifier, teardownPomodoroNotifier } from "./pomodoro-notifier";
import { isUsageTrackerEnabled, openSettings } from "./settings-window";

type AwaitArkReady = (timeoutMs?: number) => Promise<any>;
type DictationArkClient = {
  invokeOperation<T = unknown>(input: { operation: string }): Promise<T>;
};

let dictationHotkeyCache: string | null = null;

async function resolveLiveDictationShortcut(
  getArkClient: () => DictationArkClient | null,
  timeoutMs = 1500,
): Promise<string | undefined> {
  const client = getArkClient();
  if (!client) return dictationHotkeyCache ?? undefined;
  let timer: NodeJS.Timeout | null = null;
  try {
    const resp = await Promise.race([
      client.invokeOperation<{ config?: { hotkey?: string } }>({
        operation: "dictation.get_config",
      }),
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error("dictation shortcut timeout")), timeoutMs);
      }),
    ]);
    const hotkey = resp?.config?.hotkey;
    const normalized = typeof hotkey === "string" && hotkey.trim() ? hotkey.trim() : undefined;
    if (normalized) {
      dictationHotkeyCache = normalized;
      return normalized;
    }
    return dictationHotkeyCache ?? undefined;
  } catch {
    return dictationHotkeyCache ?? undefined;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

function setDictationHotkeyCache(hotkey?: string | null): void {
  dictationHotkeyCache = typeof hotkey === "string" && hotkey.trim() ? hotkey.trim() : null;
}

export function setupMainFocusRuntime(awaitArkReady: AwaitArkReady): void {
  setFocusWidgetFocusSessionOpener(openFocusSessionShell);
  setFocusWidgetRuntime({ awaitArkReady });
}

export function setupMainDictationRuntime(options: {
  awaitArkReady: AwaitArkReady;
  broadcastCommandsUpdated(): void;
  getArkClient(): DictationArkClient | null;
  invokeDictationCommand(): Promise<void>;
}): void {
  setDictationShortcutResolver(() => resolveLiveDictationShortcut(options.getArkClient));
  setDictationCommandInvoker(options.invokeDictationCommand);
  setDictationRuntime({
    awaitArkReady: options.awaitArkReady,
    broadcastCommandsUpdated: options.broadcastCommandsUpdated,
    setDictationHotkeyCache,
  });
}

export {
  isUsageTrackerEnabled,
  openSettings,
  setupDictationHotkey,
  setupFocusSessionBackendSync,
  setupFocusWidgetBackendSync,
  setupPomodoroNotifier,
  teardownFocusSessionBackendSync,
  teardownFocusWidgetBackendSync,
  teardownPomodoroNotifier,
};
