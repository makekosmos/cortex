// ---------------------------------------------------------------------------

// Enums

// ---------------------------------------------------------------------------

export enum Priority {
  None = 0,

  Low = 1,

  Medium = 2,

  High = 3,
}

export enum ProjectStatus {
  Active = 0,

  Someday = 1,

  Completed = 2,
}

export enum Frequency {
  Daily = 0,

  Weekly = 1,

  Monthly = 2,

  Yearly = 3,
}

export enum RecurrenceType {
  Fixed = 0,

  AfterCompletion = 1,
}

export enum SmartList {
  Inbox = "inbox",

  Today = "today",

  Upcoming = "upcoming",

  Anytime = "anytime",

  Someday = "someday",

  Logbook = "logbook",

  Trash = "trash",
}

// ---------------------------------------------------------------------------

// Data models

// ---------------------------------------------------------------------------

export type RecurrenceData = {
  frequency: Frequency;

  interval: number;

  recurrenceType: RecurrenceType;

  daysOfWeek?: number[] | null;

  endDate?: string | null; // ISO date string
};

export type ChecklistItem = {
  id: string;

  title: string;

  isCompleted: boolean;

  sortOrder: number;

  todoItemId?: string | null;
};

export type Tag = {
  id: string;

  title: string;

  color: string;

  shortcut?: string | null;
};

export type Heading = {
  id: string;

  title: string;

  sortOrder: number;

  projectId?: string | null;
};

export type Area = {
  id: string;

  title: string;

  sortOrder: number;

  isVisible: boolean;
};

export type Project = {
  id: string;

  title: string;

  notes?: string | null;

  status: ProjectStatus;

  scheduledDate?: string | null; // ISO date string

  deadline?: string | null;

  sortOrder: number;

  colorTag?: string | null;

  createdAt: string;

  areaId?: string | null;

  billable: boolean;

  price?: number | null;
};

export type TodoItem = {
  id: string;

  title: string;

  notes?: string | null;

  priority: Priority;

  scheduledDate?: string | null; // ISO date string

  deadline?: string | null;

  reminderDate?: string | null;

  isToday: boolean;

  isEvening: boolean;

  isSomeday: boolean;

  /**
   * Linear-style жизненный цикл задачи. Cross-app поле — пишут И Eden И Delphi
   * (двунаправленный sync через ARK `propsJson.status`). Возможные значения:
   * `triage`, `backlog`, `todo`, `done`, `canceled`. Eden's `normalizeStatus`
   * приоритезирует `status` над `is_completed` флагом, поэтому Delphi'евские
   * state transitions (`markCompleted`/`markCancelled`/`markIncomplete`)
   * ОБЯЗАНЫ синхронизировать status — иначе Eden видит несогласованную пару
   * (`status: "todo"`, `is_completed: true`) и продолжает рендерить открытую.
   *
   * `backlog` → задача показывается в SmartList «Когда-нибудь» (наряду с legacy
   * `isSomeday=true`). См. todoFilterService.ts → SmartList.Someday.
   */
  status?: string | null;

  isCompleted: boolean;

  completedAt?: string | null;

  isCancelled: boolean;

  cancelledAt?: string | null;

  isTrashed: boolean;

  sortOrder: number;

  createdAt: string;

  headingId?: string | null;

  projectId?: string | null;

  areaId?: string | null;

  tagIds: string[];

  checklistItems: ChecklistItem[];

  recurrenceRule?: RecurrenceData | null;

  billable: boolean;

  price?: number | null;
};
