export function isElectronRuntime() {
  if (typeof window === "undefined") return false;
  return Boolean((window as Window & { electronAPI?: unknown }).electronAPI);
}
