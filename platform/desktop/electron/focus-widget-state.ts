export interface FocusState {
  active: boolean;
  remainingSec: number;
  totalSec: number;
  label: string;
  mode: "work" | "break" | "stopwatch";
  /** Применён ли активный блоклист (focus mode blocking). Управляет 🛡️ индикатором в widget. */
  blockingActive: boolean;
  /** Pomodoro session на паузе. Виджет показывает Play вместо Pause. Для
      stopwatch / idle всегда false. */
  isPaused: boolean;
  /**
   * Wallclock (Unix ms) когда текущая фаза должна закончиться. null = idle/paused
   * (нет автономного тика). Когда задан и active=true, main process сам
   * пересчитывает remainingSec каждую секунду — поэтому виджет продолжает
   * тикать даже если renderer скрыт / закрыт и Chromium throttle'ит
   * его таймеры. На каждый setState от renderer'а перезаписываем — он
   * authoritative.
   */
  phaseEndsAtMs: number | null;
}

export const DEFAULT_FOCUS_WIDGET_STATE: FocusState = {
  active: false,
  remainingSec: 0,
  totalSec: 0,
  label: "",
  mode: "work",
  blockingActive: false,
  isPaused: false,
  phaseEndsAtMs: null,
};

export interface PomodoroEventState {
  phase?: "idle" | "work" | "shortBreak" | "longBreak";
  remainingMs?: number;
  totalMs?: number;
  isRunning?: boolean;
  isPaused?: boolean;
  phaseEndsAtMs?: number | null;
  title?: string;
  tasks?: Array<{ id?: string; title?: string }>;
}

export function isDefaultFocusLabel(label: string | undefined): boolean {
  const normalized = (label ?? "").trim();
  return normalized === "" || normalized === "Фокус" || normalized === "Перерыв";
}

export function deriveFocusStateFromBackend(raw: PomodoroEventState): Partial<FocusState> {
  const phase = raw.phase ?? "idle";
  const isRunning = raw.isRunning === true;
  const isPaused = raw.isPaused === true;
  const active = isRunning && !isPaused && phase !== "idle";

  // Виджет остаётся видим пока pomodoro session не idle. Это включает:
  //   - running (work/break),
  //   - paused (показываем Play),
  //   - между фазами с auto_start_*=false (Finished пришёл, isRunning=false,
  //     но phase=Work/ShortBreak/LongBreak ждёт ручного Skip/Resume).
  // Backend сбрасывает phase в Idle только на stop() — это и есть единственное
  // условие скрытия виджета.
  const widgetActive = phase !== "idle";

  const mode: FocusState["mode"] = phase === "work" ? "work" : "break";
  const remainingSec = Math.max(0, Math.ceil((raw.remainingMs ?? 0) / 1000));
  const totalSec = Math.max(0, Math.ceil((raw.totalMs ?? 0) / 1000));

  const title = (raw.title ?? "").trim();
  const firstTask = raw.tasks?.[0];
  const label = title || firstTask?.title || (mode === "work" ? "Фокус" : "Перерыв");

  return {
    active: widgetActive,
    remainingSec,
    totalSec,
    label,
    mode,
    isPaused,
    // backend не знает про focus blocklist — оставляем как есть. Renderer
    // обновит при следующем push'е если focus-session открыт.
    phaseEndsAtMs: active ? (raw.phaseEndsAtMs ?? null) : null,
  };
}
