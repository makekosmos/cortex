import { ref } from "vue";
import type { DelphiTask } from "@shared/ipc-types";

// Простой signal для оповещения view'ов про апдейт списка time entries.
// Любая операция (create/stop/update/delete) должна вызвать notifyEntriesChanged().
// Views с подпиской (ListView, App.vue) перечитают данные через watcher.
export const entriesChangedAt = ref(0);

export function notifyEntriesChanged(): void {
  entriesChangedAt.value = Date.now();
}

// Контекст top-bar'а (description / task), шарится между App.vue (top-bar)
// и любыми другими view'ами, которым нужно его прочитать.
export const currentDraft = ref<{
  title: string;
  taskId: string | null;
  taskTitle: string | null;
}>({ title: "", taskId: null, taskTitle: null });

// Контекст pomodoro-сегмента — отдельный от top-bar'а. Поддерживает несколько
// задач: на finish work-сегмента время делится поровну между ними.
export interface PomodoroDraftTask {
  id: string;
  title: string;
}

export const pomodoroDraft = ref<{
  title: string;
  tasks: PomodoroDraftTask[];
}>({ title: "", tasks: [] });

// Кэш задач Delphi (`task_obj`) для @-mention. Грузится лениво при первом обращении,
// рефрешится при необходимости через `refreshTasks()`.
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

/**
 * Гарантирует свежий список задач при открытии @-mention. Если кэш ещё пустой —
 * блокирующая загрузка (через `loadTasksOnce`); если уже есть — фоновый
 * `refreshTasks` без await, чтобы меню сразу показалось с прошлым списком,
 * а через момент обновилось свежими данными (новые задачи из Delphi подхватятся).
 */
export function ensureFreshTasks(): Promise<void> {
  if (!tasksLoaded.value) return loadTasksOnce();
  void refreshTasks();
  return Promise.resolve();
}
