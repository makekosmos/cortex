// Wire types для Horologion как Vue extension Kepler shell.
//
// В legacy `apps/horologion/shared/ipc-types.ts` эти типы описывали IPC
// контракт между main и renderer. В extension'е main-процесса больше нет;
// все ARK-операции идут через `window.kepler.ark.request(operation, params)`.
// Типы остаются полезными как DTO для всех composable / view'ов.

export type TimeEntrySource = "manual" | "imported" | "pomodoro" | "pomodoro_break";

export interface TimeEntry {
  id: string;
  title: string;
  startedAt: string; // ISO
  endedAt: string | null;
  source: TimeEntrySource;
  billable: boolean;
  tagIds: string[];
  taskId: string | null;
  /** Заголовок связанной Delphi-задачи (cached в propsJson для рендера). */
  taskTitle: string | null;
  /**
   * true только если pomodoro work-фаза дошла до конца (natural finish).
   * false / undefined — отмена/стоп/пауза/незавершённый, либо source != "pomodoro".
   * Используется для дневного счётчика «помидоров за сегодня».
   */
  completed?: boolean;
}

export interface Tag {
  id: string;
  name: string;
  color: string | null;
}

export interface StartTimerInput {
  title: string;
  tagIds?: string[];
  taskId?: string | null;
  taskTitle?: string | null;
  billable?: boolean;
  source?: TimeEntrySource;
}

export interface UpdateTimeEntryInput {
  id: string;
  title?: string;
  startedAt?: string;
  endedAt?: string | null;
  billable?: boolean;
  taskId?: string | null;
  taskTitle?: string | null;
  completed?: boolean;
}

export interface CreateTimeEntryInput {
  title: string;
  startedAt: string;
  endedAt: string;
  billable?: boolean;
  taskId?: string | null;
  taskTitle?: string | null;
  source?: TimeEntrySource;
  completed?: boolean;
}

export interface DelphiTask {
  id: string;
  title: string;
  status: string | null;
}

/**
 * Команда, инвокнутая через ARK command bus (Kosmos global launcher).
 * Renderer подписывается на `command_invoked` и сам решает, какой kind
 * проассоциировать с каждым id.
 */
export type HorologionCommandEvent =
  | { kind: "pomodoro:start"; durationMin: number }
  | { kind: "stopwatch:start" };
