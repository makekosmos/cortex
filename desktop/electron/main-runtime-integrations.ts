import { setDictationShortcutResolver } from "./commands";
import { setDictationRuntime, setupDictationHotkey } from "./dictation-pill";
import { isString } from "../src/shared/runtimeGuards";

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
    const normalized = isString(hotkey) && hotkey.trim() ? hotkey.trim() : undefined;
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
  dictationHotkeyCache = isString(hotkey) && hotkey.trim() ? hotkey.trim() : null;
}

export function setupMainDictationRuntime(options: {
  awaitArkReady: AwaitArkReady;
  broadcastCommandsUpdated(): void;
  getArkClient(): DictationArkClient | null;
}): void {
  setDictationShortcutResolver(() => resolveLiveDictationShortcut(options.getArkClient));
  setDictationRuntime({
    awaitArkReady: options.awaitArkReady,
    broadcastCommandsUpdated: options.broadcastCommandsUpdated,
    setDictationHotkeyCache,
  });
}

export { setupDictationHotkey };
