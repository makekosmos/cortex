// Helper'ы для чтения / записи `<data_dir>/pomodoro-state.json` из тестов.
//
// Файл пишется backend'ом (см. `services/kepler-backend/src/pomodoro_host.rs`)
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

export const POMODORO_STATE_FILENAME = "pomodoro-state.json";

export type Phase = "idle" | "work" | "shortBreak" | "longBreak";

export interface SessionConfigShape {
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

export function readPomodoroStateFile(dataDir: string): PomodoroStateFileShape | null {
  const p = statePath(dataDir);
  if (!fs.existsSync(p)) return null;
  const raw = fs.readFileSync(p, "utf8");
  return JSON.parse(raw) as PomodoroStateFileShape;
}

/** Атомарная запись (temp + rename) — mirror Rust save_state. */
export function writePomodoroStateFile(dataDir: string, state: PomodoroStateFileShape): void {
  if (!fs.existsSync(dataDir)) fs.mkdirSync(dataDir, { recursive: true });
  const target = statePath(dataDir);
  const tmp = target + ".tmp.test";
  fs.writeFileSync(tmp, JSON.stringify(state, null, 2), "utf8");
  fs.renameSync(tmp, target);
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
    await new Promise((r) => setTimeout(r, 100));
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
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`pomodoro-state.json не удалён из ${dataDir} за ${timeoutMs}ms`);
}
