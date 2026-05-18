import { ref } from "vue";
import type { DelphiTask } from "../types";

// Какой таймер показывается в верхнем card'е — pomodoro или обычный секундомер.
// Шарится между PomodoroView и Stopwatch view, чтобы переключение помнило выбор.
export type TimerMode = "pomodoro" | "stopwatch";
export const timerMode = ref<TimerMode>("pomodoro");

// Простой signal для оповещения view'ов про апдейт списка time entries.
export const entriesChangedAt = ref(0);

export function notifyEntriesChanged(): void {
  entriesChangedAt.value = Date.now();
}

// Контекст pomodoro/секундомер draft'а — title + список выбранных задач.
export interface PomodoroDraftTask {
  id: string;
  title: string;
}

export const pomodoroDraft = ref<{
  title: string;
  tasks: PomodoroDraftTask[];
  /** id блоклиста из ARK `blocklist_obj`. `null` ⇒ блокировка не активируется. */
  focusProfileId: string | null;
}>({ title: "", tasks: [], focusProfileId: null });

// Кэш задач Delphi (`task_obj`) для @-mention.
export const tasks = ref<DelphiTask[]>([]);
const tasksLoaded = ref(false);
let tasksPromise: Promise<void> | null = null;

export function loadTasksOnce(): Promise<void> {
  if (tasksLoaded.value) return Promise.resolve();
  if (tasksPromise) return tasksPromise;
  tasksPromise = window.horologion.tasks
    .list()
    .then((list) => {
      tasks.value = list;
      tasksLoaded.value = true;
    })
    .catch(() => {
      tasks.value = [];
      tasksLoaded.value = true;
    })
    .finally(() => {
      tasksPromise = null;
    });
  return tasksPromise;
}

export async function refreshTasks(): Promise<void> {
  tasksLoaded.value = false;
  tasksPromise = null;
  await loadTasksOnce();
}

export function ensureFreshTasks(): Promise<void> {
  if (!tasksLoaded.value) return loadTasksOnce();
  void refreshTasks();
  return Promise.resolve();
}
