import path from "node:path";
import type { Instance } from "../../desktop/electron/instance";

type Env = Record<string, string | undefined>;

export type DesktopUpdateBridge = {
  executable: string;
  stateFile: string;
};

function samePath(left: string, right: string, platform: NodeJS.Platform): boolean {
  const normalize = (value: string) => path.resolve(value);
  if (platform === "win32") return normalize(left).toLowerCase() === normalize(right).toLowerCase();
  return normalize(left) === normalize(right);
}

export function resolveDesktopUpdateBridge(input: {
  env: Env;
  instance: Instance;
  dataDir: string;
  resourcesPath: string;
  platform: NodeJS.Platform;
  isPackaged: boolean;
  exists: (file: string) => boolean;
}): DesktopUpdateBridge | null {
  const { env, instance, dataDir, resourcesPath, platform, isPackaged, exists } = input;
  if (
    platform !== "win32" ||
    !isPackaged ||
    instance.kind !== "prod" ||
    !instance.autoupdaterEnabled
  )
    return null;

  const stateFile = path.resolve(dataDir, "update-state.json");
  const executable = path.resolve(
    resourcesPath,
    "..",
    "..",
    "..",
    "..",
    `${instance.productName}.exe`,
  );
  const configuredStateFile = env.KOSMOS_UPDATE_STATE_FILE?.trim();
  const configuredExecutable = env.KOSMOS_APP_EXECUTABLE?.trim();
  if (
    !configuredStateFile ||
    !configuredExecutable ||
    !samePath(configuredStateFile, stateFile, platform) ||
    !samePath(configuredExecutable, executable, platform) ||
    !exists(executable)
  )
    return null;

  return { executable, stateFile };
}
