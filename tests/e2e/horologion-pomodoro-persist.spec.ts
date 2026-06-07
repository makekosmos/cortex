// Backend persist Pomodoro session в `<KOSMOS_DATA_DIR>/pomodoro-state.json`.
//
// Фича A: kepler-backend пишет state на каждый mutation (start/pause/resume/
// skip/stop) + на каждый pomodoro_phase_changed. На startup читает файл и
// восстанавливает Session, advance'я истёкшие фазы. Stop удаляет файл.
//
// Файлы:
//   - core/ark/crates/ark-core/rust/src/pomodoro/session.rs::PersistedSession + from_persisted
//   - platform/runtime/src/pomodoro_host.rs::PomodoroHost::new(data_dir)
//
// Тесты используют ОДИН `dataDir` между launch'ами (freshDataDir один раз).

import { test, expect } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir } from "./helpers/launch";
import { gracefulQuit, getPomodoroState, startPomodoroViaArk } from "./helpers/horologion";
import {
  pomodoroStateFileExists,
  readPomodoroStateFile,
  waitForPomodoroStateFile,
  waitForPomodoroStateFileAbsent,
  writePomodoroStateFile,
} from "./helpers/pomodoro-state-file";

// Persist + restart сценарии нуждаются в большом таймауте — два launch'а.
test.describe.configure({ mode: "serial" });

test("pomodoro persist: start пишет state-файл с running:true / phase:work", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("pomodoro-persist-start");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    // Backend boot — ARK ready.
    await new Promise((r) => setTimeout(r, 2500));
    const beforeStart = Date.now();
    const startState = await startPomodoroViaArk(app, { workMin: 25 });
    expect(startState).not.toBeNull();
    expect(startState!.phase).toBe("work");
    expect(startState!.isRunning).toBe(true);

    const persisted = await waitForPomodoroStateFile(dataDir, 3_000);
    expect(persisted.version).toBe(1);
    expect(persisted.isRunning).toBe(true);
    expect(persisted.phase).toBe("work");
    expect(persisted.phaseEndsAtMs).toBeGreaterThan(beforeStart);
  } finally {
    await gracefulQuit(app);
  }
});

test("pomodoro persist: state восстанавливается после перезапуска Kepler", async () => {
  test.setTimeout(90_000);
  const dataDir = freshDataDir("pomodoro-persist-restore");

  // --- Run 1: запускаем pomodoro, ждём persist, gracefully quit. -----------
  const app1 = await launchKeplerWithDataDir(dataDir);
  let phaseEndsAtMsBeforeQuit: number;
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app1, { workMin: 25 });
    const persisted = await waitForPomodoroStateFile(dataDir, 3_000);
    expect(persisted.isRunning).toBe(true);
    phaseEndsAtMsBeforeQuit = persisted.phaseEndsAtMs;
  } finally {
    await gracefulQuit(app1);
  }

  // --- Run 2: тот же dataDir, ожидаем что backend restore'ит session. ------
  const app2 = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 3000));
    const state = await getPomodoroState(app2);
    expect(state, "pomodoro.get_state не вернул state после restart").not.toBeNull();
    expect(state!.isRunning).toBe(true);
    expect(state!.phase).toBe("work");
    // phaseEndsAtMs должен совпадать с тем что был перед quit (wallclock
    // anchor НЕ пересчитывается на restore для running session).
    expect(state!.phaseEndsAtMs).toBe(phaseEndsAtMsBeforeQuit);
  } finally {
    await gracefulQuit(app2);
  }
});

test("pomodoro persist: advance через downtime — истёкшая work-фаза → shortBreak", async () => {
  test.setTimeout(90_000);
  const dataDir = freshDataDir("pomodoro-persist-advance");

  const app1 = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app1, { workMin: 25 });
    await waitForPomodoroStateFile(dataDir, 3_000);
  } finally {
    await gracefulQuit(app1);
  }

  // Эмулируем downtime: вручную сдвигаем phaseEndsAtMs в прошлое.
  const persisted = readPomodoroStateFile(dataDir);
  expect(persisted).not.toBeNull();
  const patched = {
    ...persisted!,
    phaseEndsAtMs: Date.now() - 1000, // фаза истекла секунду назад
    remainingMs: 0,
  };
  writePomodoroStateFile(dataDir, patched);

  // --- Run 2: backend должен advance'нуть до ShortBreak (auto_start_break=false). ---
  const app2 = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 3000));
    const state = await getPomodoroState(app2);
    expect(state).not.toBeNull();
    expect(state!.phase).toBe("shortBreak");
    expect(state!.completedPomodoros).toBe(1);
    // auto_start_break=false → break фаза не запущена, ждёт ручного старта.
    expect(state!.isRunning).toBe(false);
  } finally {
    await gracefulQuit(app2);
  }
});

test("pomodoro persist: stop удаляет state-файл", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("pomodoro-persist-stop-removes");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    await waitForPomodoroStateFile(dataDir, 3_000);
    expect(pomodoroStateFileExists(dataDir)).toBe(true);

    // Stop через ARK op.
    await app.evaluate(async ({ BrowserWindow }) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) return;
      await launcher.webContents.executeJavaScript(
        `window.kepler.ark.request("pomodoro.stop", null)`,
      );
    });

    await waitForPomodoroStateFileAbsent(dataDir, 3_000);
    expect(pomodoroStateFileExists(dataDir)).toBe(false);
  } finally {
    await gracefulQuit(app);
  }
});

test("pomodoro persist: paused state переживает restart (no advance)", async () => {
  test.setTimeout(90_000);
  const dataDir = freshDataDir("pomodoro-persist-paused");

  const app1 = await launchKeplerWithDataDir(dataDir);
  let remainingMsBeforeQuit: number;
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app1, { workMin: 25 });
    // Подождём чтобы tick уменьшил remainingMs хоть на 1s.
    await new Promise((r) => setTimeout(r, 1500));
    // Pause.
    await app1.evaluate(async ({ BrowserWindow }) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) return;
      await launcher.webContents.executeJavaScript(
        `window.kepler.ark.request("pomodoro.pause", null)`,
      );
    });
    // Ждём persist после pause.
    await new Promise((r) => setTimeout(r, 500));
    const persisted = readPomodoroStateFile(dataDir);
    expect(persisted).not.toBeNull();
    expect(persisted!.isPaused).toBe(true);
    expect(persisted!.phase).toBe("work");
    remainingMsBeforeQuit = persisted!.remainingMs;
  } finally {
    await gracefulQuit(app1);
  }

  const app2 = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 3000));
    const state = await getPomodoroState(app2);
    expect(state).not.toBeNull();
    expect(state!.isPaused).toBe(true);
    expect(state!.phase).toBe("work");
    // Paused state не advance'ится через downtime — remainingMs сохранилось.
    // Допустимая погрешность ±1s (хотя на paused tick'и no-op).
    expect(Math.abs(state!.remainingMs - remainingMsBeforeQuit)).toBeLessThan(1500);
  } finally {
    await gracefulQuit(app2);
  }
});
