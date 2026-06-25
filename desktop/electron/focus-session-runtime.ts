import type { ArkClient } from "@kosmos/ark";

type FocusSessionRuntime = {
  awaitArkReady: () => Promise<ArkClient>;
};

let shellOpener: (() => void) | null = null;
let blockedAppNotifier:
  | ((app: { id: string; title: string; icon?: string | null }) => void)
  | null = null;
let focusSessionRuntime: FocusSessionRuntime | null = null;

export function setFocusSessionShellOpener(opener: () => void): void {
  shellOpener = opener;
}

export function setBlockedAppNotifier(
  notifier: (app: { id: string; title: string; icon?: string | null }) => void,
): void {
  blockedAppNotifier = notifier;
}

export function getBlockedAppNotifier():
  | ((app: { id: string; title: string; icon?: string | null }) => void)
  | null {
  return blockedAppNotifier;
}

export function setFocusSessionRuntime(runtime: FocusSessionRuntime | null): void {
  focusSessionRuntime = runtime;
}

export function requireFocusSessionRuntime(): FocusSessionRuntime {
  if (!focusSessionRuntime) {
    throw new Error("focus session runtime bridge is not initialized");
  }
  return focusSessionRuntime;
}

export function openFocusSessionShell(): void {
  if (shellOpener) {
    shellOpener();
    return;
  }
  console.warn("[focus-session] shell opener is not registered");
}
