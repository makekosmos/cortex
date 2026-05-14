// Wire types for IPC между Electron main и renderer.
// Horologion хранит time entries как ARK time_entry_obj.

export type TimeEntrySource = "manual" | "imported";

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
}

export interface UpdateTimeEntryInput {
  id: string;
  title?: string;
  startedAt?: string;
  endedAt?: string | null;
  billable?: boolean;
  taskId?: string | null;
  taskTitle?: string | null;
}

export interface CreateTimeEntryInput {
  title: string;
  startedAt: string;
  endedAt: string;
  billable?: boolean;
  taskId?: string | null;
  taskTitle?: string | null;
}

export interface DelphiTask {
  id: string;
  title: string;
  status: string | null;
}

export type ArkConnectionStatus = "connecting" | "connected" | "error";

export interface ArkStatus {
  status: ArkConnectionStatus;
  message?: string;
  /** Путь к БД, в которую открыт sidecar. Полезно для tooltip. */
  dbPath?: string;
}

export interface HorologionApi {
  timeEntries: {
    list(): Promise<TimeEntry[]>;
    startTimer(input: StartTimerInput): Promise<TimeEntry>;
    stopTimer(id: string): Promise<TimeEntry>;
    update(input: UpdateTimeEntryInput): Promise<TimeEntry>;
    /** Полная запись с произвольным start/end. Для split'а pomodoro на N задач. */
    create(input: CreateTimeEntryInput): Promise<TimeEntry>;
    delete(id: string): Promise<void>;
    listRunning(): Promise<TimeEntry[]>;
  };
  tags: {
    list(): Promise<Tag[]>;
  };
  tasks: {
    /** Задачи из Delphi (`task_obj` в той же ARK space). Для @-mention. */
    list(): Promise<DelphiTask[]>;
  };
  ark: {
    /** Текущий статус подключения к ark-core-rpc sidecar. */
    status(): Promise<ArkStatus>;
  };
  settings: {
    /** Открыть отдельное окно настроек (или сфокусировать уже открытое). */
    open(): Promise<void>;
  };
  streamerMode: {
    /**
     * Сохранить флаг «режим стримера» на диск main-процесса. Применится после
     * перезапуска: switches `disable-features=CalculateNativeWinOcclusion` и
     * `disable-backgrounding-occluded-windows` ставятся до `app.ready`.
     */
    set(enabled: boolean): Promise<void>;
  };
}
