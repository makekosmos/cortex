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
import { randomUUID } from "node:crypto";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import { createCrashMetadata, redactText, redactUnknown } from "./redaction";

type LogLevel = "INFO" | "WARN" | "ERROR";

let cachedLogFilePath: string | null = null;
let cachedLogFileDate: string | null = null;
let correlationId = validUuid(process.env.KOSMOS_CORRELATION_ID) ?? randomUUID();

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
  const safeMeta = (redactUnknown(meta ?? {}) ?? {}) as Record<string, unknown>;
  const line =
    JSON.stringify({
      ...safeMeta,
      t: new Date().toISOString(),
      level,
      scope: redactText(scope),
      msg: redactText(msg),
      correlationId,
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
  crash(component: string, meta?: object): string {
    const crash = createCrashMetadata(component, correlationId, meta);
    write("ERROR", "crash", "process terminated", crash);
    return String(crash.crashId);
  },
  setCorrelationId(value: string): void {
    const valid = validUuid(value);
    if (valid) correlationId = valid;
  },
  correlationId(): string {
    return correlationId;
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

function validUuid(value: string | undefined): string | null {
  return value &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)
    ? value
    : null;
}
