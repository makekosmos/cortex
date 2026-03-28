// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

export enum Priority {
  None = 0,
  Low = 1,
  Medium = 2,
  High = 3,
}

export const PriorityLabel: Record<Priority, string> = {
  [Priority.None]: "Нет",
  [Priority.Low]: "Низкий",
  [Priority.Medium]: "Средний",
  [Priority.High]: "Высокий",
};

export const PriorityColor: Record<Priority, string> = {
  [Priority.None]: "gray",
  [Priority.Low]: "green",
  [Priority.Medium]: "orange",
  [Priority.High]: "red",
};

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

export const SmartListTitle: Record<SmartList, string> = {
  [SmartList.Inbox]: "Входящие",
  [SmartList.Today]: "Сегодня",
  [SmartList.Upcoming]: "Планы",
  [SmartList.Anytime]: "Когда угодно",
  [SmartList.Someday]: "Потом",
  [SmartList.Logbook]: "Журнал",
  [SmartList.Trash]: "Корзина",
};

// ---------------------------------------------------------------------------
// Data models
// ---------------------------------------------------------------------------

export type RecurrenceData = {
  frequency: Frequency;
  interval: number;
  recurrenceType: RecurrenceType;
  daysOfWeek?: number[] | null;
  endDate?: string | null;
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
  scheduledDate?: string | null;
  deadline?: string | null;
  sortOrder: number;
  colorTag?: string | null;
  createdAt: string;
  areaId?: string | null;
};

export type TodoItem = {
  id: string;
  title: string;
  notes?: string | null;
  priority: Priority;
  scheduledDate?: string | null;
  deadline?: string | null;
  reminderDate?: string | null;
  isToday: boolean;
  isEvening: boolean;
  isSomeday: boolean;
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
};
