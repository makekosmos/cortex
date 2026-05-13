import { reactive, watch } from "vue";
import type { SoundName } from "./sounds";

export interface PomodoroSettings {
  /** Длительность рабочего фокус-сегмента, минуты. */
  workMin: number;
  /** Короткий брейк после фокус-сегмента, минуты. */
  shortBreakMin: number;
  /** Длинный брейк после `pomodorosUntilLongBreak` сегментов, минуты. */
  longBreakMin: number;
  /** Сколько work-сегментов до длинного брейка. */
  pomodorosUntilLongBreak: number;
  /**
   * Трекать ли break-фазы как отдельный `time_entry_obj` с тегом «Отдых».
   * Если false — break идёт как чистый таймер без записи в ARK.
   */
  trackBreaksAsRest: boolean;
  /** Автоматически стартовать work-сегмент после break'а. */
  autoStartWork: boolean;
  /** Автоматически стартовать break-сегмент после work'а. */
  autoStartBreak: boolean;
  /** Системные уведомления (через Web Notification API) при смене фазы. */
  systemNotifications: boolean;
  /** Звук в конце work-сегмента. */
  workEndSound: SoundName;
  /** Звук в конце break-сегмента. */
  breakEndSound: SoundName;
}

export const DEFAULT_POMODORO_SETTINGS: PomodoroSettings = {
  workMin: 25,
  shortBreakMin: 5,
  longBreakMin: 15,
  pomodorosUntilLongBreak: 4,
  trackBreaksAsRest: false,
  autoStartWork: false,
  autoStartBreak: true,
  systemNotifications: true,
  workEndSound: "bell",
  breakEndSound: "chime",
};

const STORAGE_KEY = "horologion.pomodoro.settings.v1";

function load(): PomodoroSettings {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_POMODORO_SETTINGS };
    const parsed = JSON.parse(raw) as Partial<PomodoroSettings>;
    return { ...DEFAULT_POMODORO_SETTINGS, ...parsed };
  } catch {
    return { ...DEFAULT_POMODORO_SETTINGS };
  }
}

// Singleton reactive store. Все view'и работают с одним и тем же объектом.
export const pomodoroSettings = reactive<PomodoroSettings>(load());

watch(
  pomodoroSettings,
  (next) => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* ignore */
    }
  },
  { deep: true },
);

export function resetPomodoroSettings(): void {
  Object.assign(pomodoroSettings, DEFAULT_POMODORO_SETTINGS);
}
