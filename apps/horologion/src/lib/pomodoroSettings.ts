import { reactive, watch } from "vue";
import { setVolumeMultiplier, type SoundName } from "./sounds";

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
  /**
   * Отключить паузу рендеринга при перекрытии окна другим окном (для стримов).
   * Применяется после перезапуска: main процессу нужны command-line switches
   * до `app.ready`.
   */
  streamerMode: boolean;
  /** Звук в конце work-сегмента. */
  workEndSound: SoundName;
  /** Звук в конце break-сегмента. */
  breakEndSound: SoundName;
  /** Громкость рингтона, 0..1. Умножается на peak gain каждого тона. */
  ringtoneVolume: number;
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
  streamerMode: false,
  workEndSound: "bell",
  breakEndSound: "chime",
  ringtoneVolume: 1,
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

// Initial sync громкости с sounds-модулем.
setVolumeMultiplier(pomodoroSettings.ringtoneVolume);

watch(
  pomodoroSettings,
  (next) => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* ignore */
    }
    setVolumeMultiplier(next.ringtoneVolume);
    void window.horologion?.streamerMode?.set(next.streamerMode);
  },
  { deep: true },
);

void window.horologion?.streamerMode?.set(pomodoroSettings.streamerMode);

export function resetPomodoroSettings(): void {
  Object.assign(pomodoroSettings, DEFAULT_POMODORO_SETTINGS);
}
