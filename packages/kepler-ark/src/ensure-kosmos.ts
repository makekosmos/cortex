// Phase 2: ensureKosmosRunning() — shared helper для всех Electron-апок.
//
// Логика (см. план Decision #4 + Phase 6 AC5-AC8):
//   1. Прочитать %APPDATA%\Kepler\kosmos.lock.json
//   2. Если есть и PID жив + protocol_version.major матчит — connected
//   3. Если lock есть, но PID мёртв → удалить stale lock, продолжить
//   4. Если lock нет — проверить conventional paths kosmos exe
//   5. Если exe нет — not-installed (апка показывает modal "Скачать Kosmos")
//   6. Если exe есть — spawn detached, polling lock-файл до waitMs
//   7. Если lock появился — connected; иначе launch-failed
//
// Это shared между Eden / Delphi / Arrancador / Horologion / Dashboard, чтобы
// 5 апок не дублировали ~100 строк lock-file parsing + spawn + polling логики.

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { getKeplerDataDir } from "./selected-space.js";

export const KOSMOS_LOCK_FILENAME = "kosmos.lock.json";

export interface KosmosProtocolVersion {
  major: number;
  minor: number;
  patch: number;
}

/**
 * Структура lock-файла, который пишет Rust-бинарь kosmos. Поля snake_case —
 * serde по умолчанию (см. apps/kosmos/src/lock_file.rs::KosmosLockFile).
 */
export interface KosmosLockInfo {
  format_version: number;
  protocol_version: KosmosProtocolVersion;
  pid: number;
  ws_port: number;
  auth_token: string;
  started_at: string;
  db_path: string;
}

export type KosmosState =
  | { kind: "connected"; lock: KosmosLockInfo; lockPath: string }
  | { kind: "not-installed"; checkedPaths: string[] }
  | { kind: "launch-failed"; reason: string }
  | {
      kind: "incompatible-version";
      cosmosVersion: KosmosProtocolVersion;
      clientMajor: number;
    };

export interface EnsureKosmosOptions {
  /** Обычно `app.getPath("appData")` (Electron) или %APPDATA% (Win) / ~/.config (Unix). */
  appDataPath: string;
  /** Максимум ms ждать lock-file после spawn. Default 10000. */
  waitMs?: number;
  /** MAJOR версия протокола, поддерживаемая клиентом. Default 1. */
  clientProtocolMajor?: number;
  /** Дополнительные пути для поиска kosmos.exe (override defaults). */
  conventionalPaths?: string[];
  /** Авто-запуск Kosmos если установлен но не запущен. Default true. */
  autoLaunch?: boolean;
}

/**
 * Главный entrypoint. Возвращает KosmosState — апка реагирует через switch.
 */
export async function ensureKosmosRunning(
  opts: EnsureKosmosOptions,
): Promise<KosmosState> {
  const waitMs = opts.waitMs ?? 10000;
  const clientMajor = opts.clientProtocolMajor ?? 1;
  const autoLaunch = opts.autoLaunch ?? true;

  const lockPath = resolveLockPath(opts.appDataPath);

  // 1. Существующий live lock?
  const existing = readLockIfAlive(lockPath);
  if (existing) {
    if (existing.protocol_version.major !== clientMajor) {
      return {
        kind: "incompatible-version",
        cosmosVersion: existing.protocol_version,
        clientMajor,
      };
    }
    return { kind: "connected", lock: existing, lockPath };
  }

  if (!autoLaunch) {
    return { kind: "not-installed", checkedPaths: [] };
  }

  // 2. Auto-launch.
  const attempted: string[] = [];
  const exe = resolveKosmosExe(opts.conventionalPaths, attempted);
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
          cosmosVersion: lock.protocol_version,
          clientMajor,
        };
      }
      return { kind: "connected", lock, lockPath };
    }
    await sleep(250);
  }

  return {
    kind: "launch-failed",
    reason: `Kosmos exe spawned (${exe}), but lock-file did not appear in ${waitMs}ms`,
  };
}

export function resolveLockPath(appDataPath: string): string {
  return path.join(getKeplerDataDir(appDataPath), KOSMOS_LOCK_FILENAME);
}

export function readLockIfAlive(lockPath: string): KosmosLockInfo | null {
  if (!fs.existsSync(lockPath)) return null;
  let lock: KosmosLockInfo;
  try {
    const content = fs.readFileSync(lockPath, "utf8");
    const parsed = JSON.parse(content) as Partial<KosmosLockInfo>;
    if (
      typeof parsed.pid !== "number" ||
      typeof parsed.ws_port !== "number" ||
      typeof parsed.auth_token !== "string" ||
      !parsed.protocol_version ||
      typeof parsed.protocol_version.major !== "number"
    ) {
      return null;
    }
    lock = parsed as KosmosLockInfo;
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

export function resolveKosmosExe(
  custom: string[] | undefined,
  attempted: string[],
): string | null {
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
    if (process.env.LOCALAPPDATA) {
      list.push(
        path.join(process.env.LOCALAPPDATA, "Kepler", "Kosmos", "kosmos.exe"),
      );
    }
    if (process.env["ProgramFiles"]) {
      list.push(
        path.join(
          process.env["ProgramFiles"],
          "Kepler",
          "Kosmos",
          "kosmos.exe",
        ),
      );
    }
  } else if (process.platform === "darwin") {
    list.push("/Applications/Kepler Kosmos.app/Contents/MacOS/kosmos");
    if (process.env.HOME) {
      list.push(
        path.join(
          process.env.HOME,
          "Applications",
          "Kepler Kosmos.app",
          "Contents",
          "MacOS",
          "kosmos",
        ),
      );
    }
  } else {
    list.push("/usr/local/bin/kosmos");
    if (process.env.HOME) {
      list.push(path.join(process.env.HOME, ".local", "bin", "kosmos"));
    }
  }

  return list;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
