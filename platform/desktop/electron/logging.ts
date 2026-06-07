// Phase 4 bug-detection: structured logging для shell main process.
//
// Что: пишет JSON-line одновременно в:
//   1. `<data_dir>/logs/<slot>/kepler-shell-<DATE>.log` (rolling daily).
//   2. process.stderr (для dev visibility + parent process pipe).
//
// Зачем: console.error("[kepler-shell] ...") уходит только в stderr,
// который пропадает после exit процесса. Реальный пользователь, который
// репортит баг, не может прислать логи — их нет на диске.
//
// API:
//   keplerLog.info("scope", "msg", { meta })
//   keplerLog.warn("scope", "msg", { err: String(e) })
//   keplerLog.error("scope", "msg", { err: String(e), stack: e.stack })
//
// Не используется в renderer/Vue — там остаётся console.* (отдельная фаза
// гигиены, не Phase 4 scope).

import { appendFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";

type LogLevel = "INFO" | "WARN" | "ERROR";

let cachedLogFilePath: string | null = null;
let cachedLogFileDate: string | null = null;

function ensureLogFilePath(): string {
  const today = new Date().toISOString().slice(0, 10);
  if (cachedLogFilePath && cachedLogFileDate === today) {
    return cachedLogFilePath;
  }
  const logDir = path.join(keplerDataDir(), "logs");
  try {
    mkdirSync(logDir, { recursive: true });
  } catch {
    // Если не можем создать dir — пишем только в stderr, не throw'ем
    // (logging не должно валить процесс).
  }
  cachedLogFilePath = path.join(logDir, `kepler-shell-${today}.log`);
  cachedLogFileDate = today;
  return cachedLogFilePath;
}

function write(level: LogLevel, scope: string, msg: string, meta?: object): void {
  const line =
    JSON.stringify({
      t: new Date().toISOString(),
      level,
      scope,
      msg,
      ...meta,
    }) + "\n";

  // 1. Stderr — для dev visibility и для parent process (если shell спавнен
  //    через Playwright, e2e harness видит stderr).
  try {
    process.stderr.write(line);
  } catch {
    // ignore — broken pipe
  }

  // 2. File — для пользователя который сделает bug report.
  try {
    appendFileSync(ensureLogFilePath(), line);
  } catch {
    // ignore — disk full, permission denied. Stderr выше уже сработал.
  }
}

export const keplerLog = {
  info(scope: string, msg: string, meta?: object): void {
    write("INFO", scope, msg, meta);
  },
  warn(scope: string, msg: string, meta?: object): void {
    write("WARN", scope, msg, meta);
  },
  error(scope: string, msg: string, meta?: object): void {
    write("ERROR", scope, msg, meta);
  },
  /** Текущий путь к лог-файлу — для bug bundle / диагностики. */
  currentLogFile(): string {
    return ensureLogFilePath();
  },
  /** Директория с логами — для bug bundle (ZIP includes everything в этой
      директории). */
  logsDir(): string {
    return path.join(keplerDataDir(), "logs");
  },
};
