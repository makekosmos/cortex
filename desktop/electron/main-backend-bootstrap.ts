import { existsSync, readFileSync, realpathSync } from "node:fs";
import path from "node:path";
import type { Instance } from "./instance";
import type { JsonRecord } from "./extension-permissions";

interface ResolveBackendExeArgs {
  dirname: string;
  env: NodeJS.ProcessEnv;
  resourcesPath: string;
  platform?: NodeJS.Platform;
}

function installedEngineBackend(env: NodeJS.ProcessEnv, platform: NodeJS.Platform): string | null {
  const configuredRoot = env.KOSMOS_ENGINE_ROOT?.trim();
  const root =
    configuredRoot ||
    (platform === "win32" && env.LOCALAPPDATA
      ? path.join(env.LOCALAPPDATA, "Kosmos", "Engine")
      : null);
  if (!root) return null;
  try {
    const pointer = JSON.parse(readFileSync(path.join(root, "current.json"), "utf8")) as {
      schema_version?: unknown;
      version?: unknown;
    };
    if (pointer.schema_version !== 1 || !isEngineVersion(pointer.version)) return null;
    const engineRoot = path.resolve(root);
    const versionsRoot = path.resolve(engineRoot, "versions");
    const backend = path.resolve(
      versionsRoot,
      pointer.version,
      platform === "win32" ? "kepler-backend.exe" : "kepler-backend",
    );
    if (!isWithinRoot(backend, versionsRoot) || !existsSync(backend)) return null;
    const realVersionsRoot = realpathSync(versionsRoot);
    const realBackend = realpathSync(backend);
    return isWithinRoot(realBackend, realVersionsRoot) ? realBackend : null;
  } catch {
    return null;
  }
}

function isEngineVersion(value: unknown): value is string {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function isWithinRoot(candidate: string, root: string): boolean {
  const relative = path.relative(root, candidate);
  return (
    relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}

export function resolveBackendExe({
  dirname,
  env,
  resourcesPath,
  platform = process.platform,
}: ResolveBackendExeArgs): string {
  const fromEnv = env.KEPLER_BACKEND_EXE;
  if (fromEnv && existsSync(fromEnv)) return fromEnv;

  const backendBin = platform === "win32" ? "kepler-backend.exe" : "kepler-backend";
  const packagedRuntime = platform === "win32" ? "Kosmos Runtime.exe" : "Kosmos Runtime";

  const devDebug = path.resolve(dirname, "../../target/debug", backendBin);
  if (existsSync(devDebug)) return devDebug;
  const devRelease = path.resolve(dirname, "../../target/release", backendBin);
  if (existsSync(devRelease)) return devRelease;

  const packaged = path.join(resourcesPath, packagedRuntime);
  if (existsSync(packaged)) return packaged;
  const installed = installedEngineBackend(env, platform);
  if (installed) return installed;
  return path.join(resourcesPath, backendBin);
}

interface BootSelfCheckLogger {
  error(scope: string, message: string, data?: JsonRecord): void;
  info(scope: string, message: string, data?: JsonRecord): void;
}

interface RunBootSelfCheckArgs {
  backendExe: string;
  env: NodeJS.ProcessEnv;
  instance: Instance;
  log: BootSelfCheckLogger;
  showErrorBox: (title: string, content: string) => void;
  exit: (code: number) => void;
  verifyUserDataMatches: (instance: Instance) => {
    ok: boolean;
    expected: string;
    actual: string;
  };
}

export function runBootSelfCheck({
  backendExe,
  env,
  exit,
  instance,
  log,
  showErrorBox,
  verifyUserDataMatches,
}: RunBootSelfCheckArgs): void {
  const verify = verifyUserDataMatches(instance);
  if (!verify.ok) {
    const msg =
      `Kosmos boot self-check failed: userData mismatch.\n` +
      `Expected: ${verify.expected}\n` +
      `Actual:   ${verify.actual}\n` +
      `Slot:     ${instance.slot}\n\n` +
      `Это означает что applyInstanceToApp не успел отработать до первого ` +
      `чтения userData path. Запустите Kosmos заново; если повторяется — ` +
      `см. platform/desktop/electron/instance.ts.`;
    log.error("boot", "userData mismatch", {
      expected: verify.expected,
      actual: verify.actual,
      slot: instance.slot,
    });
    showErrorBox("Kosmos — ошибка запуска", msg);
    exit(1);
    return;
  }

  if (!existsSync(backendExe)) {
    const msg =
      `Kosmos Runtime не найден по ожидаемому пути:\n${backendExe}\n\n` +
      `Возможно установка повреждена. Переустановите Kosmos.`;
    log.error("boot", "backend exe missing", { backendExe });
    showErrorBox("Kosmos — ошибка запуска", msg);
    exit(1);
    return;
  }

  if (instance.kind === "test" && env.KOSMOS_TEST_MODE !== "1") {
    const msg =
      `Kosmos запущен в test slot (${instance.slot}) без KOSMOS_TEST_MODE=1.\n` +
      `Это обычно означает что KOSMOS_DATA_DIR / KEPLER_INSTANCE прокинут случайно.\n` +
      `Очистите env и запустите снова.`;
    log.error("boot", "test slot without KOSMOS_TEST_MODE", {
      slot: instance.slot,
    });
    showErrorBox("Kosmos — ошибка запуска", msg);
    exit(1);
    return;
  }

  log.info("boot", "self-check passed", {
    slot: instance.slot,
    kind: instance.kind,
    userDataDir: instance.userDataDir,
    dataDir: instance.dataDir,
    backendExe,
  });
}
