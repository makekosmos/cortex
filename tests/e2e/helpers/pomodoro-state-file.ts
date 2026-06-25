// Helper'ы для чтения / записи `<data_dir>/pomodoro-state.json` из тестов.
//
// Файл пишется backend'ом (см. `platform/runtime/src/pomodoro_host.rs`)
// на каждый mutation pomodoro session. Используется e2e-тестами для:
//   1. Проверки persist'а: после start/pause/skip/stop файл существует или
//      отсутствует, содержимое соответствует ожиданиям.
//   2. Эмуляции downtime: тест может вручную сдвинуть `phaseEndsAtMs` в
//      прошлое и затем перезапустить Kepler — backend на startup advance'нет
//      истёкшую фазу.
//
// Wire-format совпадает с Rust `PersistedSession` (camelCase serde).

import fs from "node:fs";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const POMODORO_STATE_FILENAME = "pomodoro-state.json";

type Phase = "idle" | "work" | "shortBreak" | "longBreak";

interface SessionConfigShape {
  workMin: number;
  shortBreakMin: number;
  longBreakMin: number;
  pomodorosUntilLongBreak: number;
  autoStartWork?: boolean;
  autoStartBreak?: boolean;
  title?: string;
  tasks?: Array<{ id: string; title: string }>;
  workMinOverride?: number;
}

export interface PomodoroStateFileShape {
  version: number;
  phase: Phase;
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  phaseEndsAtMs: number;
  lastConfig: SessionConfigShape | null;
}

function statePath(dataDir: string): string {
  return path.join(dataDir, POMODORO_STATE_FILENAME);
}

export function pomodoroStateFileExists(dataDir: string): boolean {
  return fs.existsSync(statePath(dataDir));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isPhase(value: unknown): value is Phase {
  return value === "idle" || value === "work" || value === "shortBreak" || value === "longBreak";
}

function isSessionConfigShape(value: unknown): value is SessionConfigShape {
  if (!isRecord(value)) return false;
  return (
    typeof value.workMin === "number" &&
    typeof value.shortBreakMin === "number" &&
    typeof value.longBreakMin === "number" &&
    typeof value.pomodorosUntilLongBreak === "number"
  );
}

function parsePomodoroStateFile(raw: string, filePath: string): PomodoroStateFileShape {
  const parsed: unknown = JSON.parse(raw);
  if (
    !isRecord(parsed) ||
    typeof parsed.version !== "number" ||
    !isPhase(parsed.phase) ||
    typeof parsed.remainingMs !== "number" ||
    typeof parsed.totalMs !== "number" ||
    typeof parsed.completedPomodoros !== "number" ||
    typeof parsed.isRunning !== "boolean" ||
    typeof parsed.isPaused !== "boolean" ||
    typeof parsed.phaseEndsAtMs !== "number" ||
    (parsed.lastConfig !== null && !isSessionConfigShape(parsed.lastConfig))
  ) {
    throw new Error(`Invalid pomodoro state file shape: ${filePath}`);
  }
  return {
    version: parsed.version,
    phase: parsed.phase,
    remainingMs: parsed.remainingMs,
    totalMs: parsed.totalMs,
    completedPomodoros: parsed.completedPomodoros,
    isRunning: parsed.isRunning,
    isPaused: parsed.isPaused,
    phaseEndsAtMs: parsed.phaseEndsAtMs,
    lastConfig: parsed.lastConfig,
  };
}

function readPomodoroStateFile(dataDir: string): PomodoroStateFileShape | null {
  const p = statePath(dataDir);
  if (!fs.existsSync(p)) return null;
  const raw = fs.readFileSync(p, "utf8");
  return parsePomodoroStateFile(raw, p);
}

/** Polling-helper: дожидается появления файла в течение `timeoutMs`. */
export async function waitForPomodoroStateFile(
  dataDir: string,
  timeoutMs = 5_000,
): Promise<PomodoroStateFileShape> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const s = readPomodoroStateFile(dataDir);
    if (s) return s;
    await delay(100);
  }
  throw new Error(`pomodoro-state.json не появился в ${dataDir} за ${timeoutMs}ms`);
}

/** Polling: ждёт пока файл исчезнет (или сразу возвращает если его нет). */
export async function waitForPomodoroStateFileAbsent(
  dataDir: string,
  timeoutMs = 5_000,
): Promise<void> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (!pomodoroStateFileExists(dataDir)) return;
    await delay(100);
  }
  throw new Error(`pomodoro-state.json не удалён из ${dataDir} за ${timeoutMs}ms`);
}
