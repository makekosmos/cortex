// Phase 2: ensureKeplerRunning() — shared helper для всех Electron-апок.
//
// Логика (см. план Decision #4 + Phase 6 AC5-AC8):
//   1. Прочитать %APPDATA%\Kosmos\kepler.lock.json
//   2. Если есть и PID жив + protocol_version.major матчит — connected
//   3. Если lock есть, но PID мёртв → удалить stale lock, продолжить
//   4. Если lock нет — проверить conventional paths kepler exe
//   5. Если exe нет — not-installed (апка показывает modal "Скачать Kepler")
//   6. Если exe есть — spawn detached, polling lock-файл до waitMs
//   7. Если lock появился — connected; иначе launch-failed
//
// Это shared между Eden / Delphi / Arrancador / Dashboard, чтобы
// 5 апок не дублировали ~100 строк lock-file parsing + spawn + polling логики.

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

export const KEPLER_LOCK_FILENAME = "kepler.lock.json";

function env(name: string): string | undefined {
  return process.env[name];
}

/** Базовый Kosmos data dir под appData (ранее жил в selected-space.ts; теперь
 *  локальный helper — единственным consumer'ом был этот файл). */
function getKosmosDataDir(appDataPath: string): string {
  return path.join(appDataPath, "Kosmos");
}

export interface KeplerProtocolVersion {
  major: number;
  minor: number;
  patch: number;
}

/**
 * Структура lock-файла, который пишет Rust-бинарь kepler. Поля snake_case —
 * serde по умолчанию (см. apps/kepler/src/lock_file.rs::KeplerLockFile).
 */
export interface KeplerLockInfo {
  format_version: number;
  protocol_version: KeplerProtocolVersion;
  pid: number;
  ws_port: number;
  auth_token: string;
  started_at: string;
  db_path: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isProtocolVersion(value: unknown): value is KeplerProtocolVersion {
  return (
    isRecord(value) &&
    typeof value.major === "number" &&
    typeof value.minor === "number" &&
    typeof value.patch === "number"
  );
}

function isKeplerLockInfo(value: unknown): value is KeplerLockInfo {
  return (
    isRecord(value) &&
    typeof value.format_version === "number" &&
    isProtocolVersion(value.protocol_version) &&
    typeof value.pid === "number" &&
    typeof value.ws_port === "number" &&
    typeof value.auth_token === "string" &&
    typeof value.started_at === "string" &&
    typeof value.db_path === "string"
  );
}

export type KeplerState =
  | { kind: "connected"; lock: KeplerLockInfo; lockPath: string }
  | { kind: "not-installed"; checkedPaths: string[] }
  | { kind: "launch-failed"; reason: string }
  | {
      kind: "incompatible-version";
      keplerVersion: KeplerProtocolVersion;
      clientMajor: number;
    };

export interface EnsureKeplerOptions {
  /** Обычно `app.getPath("appData")` (Electron) или %APPDATA% (Win) / ~/.config (Unix). */
  appDataPath: string;
  /** Максимум ms ждать lock-file после spawn. Default 10000. */
  waitMs?: number;
  /** MAJOR версия протокола, поддерживаемая клиентом. Default 1. */
  clientProtocolMajor?: number;
  /** Дополнительные пути для поиска kepler.exe (override defaults). */
  conventionalPaths?: string[];
  /** Авто-запуск Kepler если установлен но не запущен. Default true. */
  autoLaunch?: boolean;
  /**
   * Прямой override базового data dir (где живёт kepler.lock.json).
   * Используется shell'ом при dev/test mode чтобы lock-файл искался под
   * %APPDATA%/Kosmos-dev/ или tests/.e2e/<slug>/ соответственно. Если не
   * задан — fallback к process.env.KOSMOS_DATA_DIR, затем к
   * getKosmosDataDir(appDataPath).
   */
  dataDir?: string;
}

/**
 * Главный entrypoint. Возвращает KeplerState — апка реагирует через switch.
 */
export async function ensureKeplerRunning(opts: EnsureKeplerOptions): Promise<KeplerState> {
  const waitMs = opts.waitMs ?? 10000;
  const clientMajor = opts.clientProtocolMajor ?? 1;
  const autoLaunch = opts.autoLaunch ?? true;

  const lockPath = opts.dataDir
    ? path.join(opts.dataDir, KEPLER_LOCK_FILENAME)
    : resolveLockPath(opts.appDataPath);

  // 1. Существующий live lock?
  const existing = readLockIfAlive(lockPath);
  if (existing) {
    if (existing.protocol_version.major !== clientMajor) {
      return {
        kind: "incompatible-version",
        keplerVersion: existing.protocol_version,
        clientMajor,
      };
    }
    return { kind: "connected", lock: existing, lockPath };
  }

  if (!autoLaunch) {
    // autoLaunch=false означает «не спавни kepler сам, я уже спавнил/он скоро
    // появится». Lock-файл может ещё не быть записан backend'ом — поллим до
    // waitMs прежде чем сдаваться. Без этого получался race: shell спавнит
    // backend → сразу зовёт ensureKeplerRunning → lock ещё не написан →
    // возвращается not-installed → ArkClient не поднимается.
    const startTime = Date.now();
    while (Date.now() - startTime < waitMs) {
      const lock = readLockIfAlive(lockPath);
      if (lock) {
        if (lock.protocol_version.major !== clientMajor) {
          return {
            kind: "incompatible-version",
            keplerVersion: lock.protocol_version,
            clientMajor,
          };
        }
        return { kind: "connected", lock, lockPath };
      }
      await sleep(250);
    }
    // autoLaunch=false = caller already spawned backend и ждёт только lock-file.
    // Если lock не появился вовремя, installation не исчезла — это startup
    // failure/timeout, а не not-installed. См. postmortems.md § 2026-06-16.
    return {
      kind: "launch-failed",
      reason: `self-managed backend did not publish lock-file ${lockPath} within ${waitMs}ms`,
    };
  }

  // 2. Auto-launch.
  const attempted: string[] = [];
  const exe = resolveKeplerExe(opts.conventionalPaths, attempted);
  if (!exe) {
    return { kind: "not-installed", checkedPaths: attempted };
  }

  try {
    const child = spawn(exe, [], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    if (child.exitCode !== null) {
      return {
        kind: "launch-failed",
        reason: `child exited immediately (code ${child.exitCode})`,
      };
    }
  } catch (e) {
    return {
      kind: "launch-failed",
      reason: `spawn failed: ${(e as Error).message}`,
    };
  }

  // 3. Poll for lock-file.
  const startTime = Date.now();
  while (Date.now() - startTime < waitMs) {
    const lock = readLockIfAlive(lockPath);
    if (lock) {
      if (lock.protocol_version.major !== clientMajor) {
        return {
          kind: "incompatible-version",
          keplerVersion: lock.protocol_version,
          clientMajor,
        };
      }
      return { kind: "connected", lock, lockPath };
    }
    await sleep(250);
  }

  return {
    kind: "launch-failed",
    reason: `Kepler exe spawned (${exe}), but lock-file did not appear in ${waitMs}ms`,
  };
}

export function resolveLockPath(appDataPath: string): string {
  // KOSMOS_DATA_DIR env override полностью замещает базовый data dir —
  // backend пишет lock-файл туда же (см. platform/runtime/src/lock_file.rs
  // kosmos_data_dir() helper). e2e тестам это обязательно — иначе ensureKepler
  // искал бы lock в %APPDATA%/Kosmos/ и говорил «not-installed» даже когда
  // backend running под тестовым dir'ом.
  const override = env("KOSMOS_DATA_DIR");
  if (override && override.length > 0) {
    return path.join(override, KEPLER_LOCK_FILENAME);
  }
  return path.join(getKosmosDataDir(appDataPath), KEPLER_LOCK_FILENAME);
}

export function readLockIfAlive(lockPath: string): KeplerLockInfo | null {
  if (!fs.existsSync(lockPath)) return null;
  let lock: KeplerLockInfo;
  try {
    const content = fs.readFileSync(lockPath, "utf8");
    const parsed: unknown = JSON.parse(content);
    if (!isKeplerLockInfo(parsed)) return null;
    lock = parsed;
  } catch {
    return null;
  }

  if (!isPidAlive(lock.pid)) {
    // Stale — best-effort cleanup, ignore errors.
    try {
      fs.unlinkSync(lockPath);
    } catch {
      /* lock-file ownership might disallow; не критично */
    }
    return null;
  }

  return lock;
}

/**
 * Проверка существования процесса по PID. На Win/Mac/Linux одинаково через
 * `process.kill(pid, 0)` (signal 0 — это zero-cost existence check).
 */
export function isPidAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

export function resolveKeplerExe(custom: string[] | undefined, attempted: string[]): string | null {
  const candidates: string[] = custom ? [...custom] : defaultExeCandidates();
  for (const c of candidates) {
    if (fs.existsSync(c)) {
      return c;
    }
    attempted.push(c);
  }
  return null;
}

function defaultExeCandidates(): string[] {
  const list: string[] = [];

  if (process.platform === "win32") {
    const localAppData = env("LOCALAPPDATA");
    if (localAppData) {
      list.push(path.join(localAppData, "Programs", "Kosmos", "Kosmos.exe"));
      list.push(path.join(localAppData, "Programs", "Kepler", "Kepler.exe"));
      list.push(path.join(localAppData, "Kosmos", "Kepler", "kepler.exe"));
    }
    const programFiles = env("ProgramFiles");
    if (programFiles) {
      list.push(path.join(programFiles, "Kosmos", "Kosmos.exe"));
      list.push(path.join(programFiles, "Kepler", "Kepler.exe"));
      list.push(path.join(programFiles, "Kosmos", "Kepler", "kepler.exe"));
    }
  } else if (process.platform === "darwin") {
    list.push("/Applications/Kosmos.app/Contents/MacOS/Kosmos");
    list.push("/Applications/Kosmos Kepler.app/Contents/MacOS/kepler");
    const home = env("HOME");
    if (home) {
      list.push(
        path.join(home, "Applications", "Kosmos Kepler.app", "Contents", "MacOS", "kepler"),
      );
    }
  } else {
    list.push("/usr/local/bin/kepler");
    const home = env("HOME");
    if (home) {
      list.push(path.join(home, ".local", "bin", "kepler"));
    }
  }

  return list;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
