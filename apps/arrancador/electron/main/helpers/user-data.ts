import os from "node:os";
import path from "node:path";

const DEFAULT_APP_NAME = "arrancador";

function resolveSharedDataRoot(): string {
  if (process.platform === "win32") {
    return process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local");
  }

  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support");
  }

  return process.env.XDG_DATA_HOME ?? path.join(os.homedir(), ".local", "share");
}

export function resolveSharedUserDataPath(appName = DEFAULT_APP_NAME): string {
  return path.join(resolveSharedDataRoot(), appName);
}

