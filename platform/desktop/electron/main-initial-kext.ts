import { findKextInArgv, openInstallExtensionWindow } from "./install-extension-window";

export function openInitialKextFromArgv(argv: string[]): void {
  const initialKext = findKextInArgv(argv);
  if (initialKext) {
    openInstallExtensionWindow(initialKext);
  }
}
