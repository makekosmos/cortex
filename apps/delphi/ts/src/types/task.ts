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

export const ProjectStatusLabel: Record<ProjectStatus, string> = {
  [ProjectStatus.Active]: "Активный",
  [ProjectStatus.Someday]: "Потом",
  [ProjectStatus.Completed]: "Завершён",
};

export enum Frequency {
  Daily = 0,
  Weekly = 1,
  Monthly = 2,
  Yearly = 3,
}

export const FrequencyLabel: Record<Frequency, string> = {
  [Frequency.Daily]: "Ежедневно",
  [Frequency.Weekly]: "Еженедельно",
  [Frequency.Monthly]: "Ежемесячно",
  [Frequency.Yearly]: "Ежегодно",
};

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

export const SmartListIcon: Record<SmartList, string> = {
  [SmartList.Inbox]: "inbox",
  [SmartList.Today]: "star",
  [SmartList.Upcoming]: "calendar",
  [SmartList.Anytime]: "layers",
  [SmartList.Someday]: "archive",
  [SmartList.Logbook]: "book",
  [SmartList.Trash]: "trash-2",
};

export const SmartListColor: Record<SmartList, string> = {
  [SmartList.Inbox]: "blue",
  [SmartList.Today]: "yellow",
  [SmartList.Upcoming]: "red",
  [SmartList.Anytime]: "purple",
  [SmartList.Someday]: "brown",
  [SmartList.Logbook]: "green",
  [SmartList.Trash]: "gray",
};

/** Keyboard shortcut for Cmd+N (null = no shortcut). */
export const SmartListShortcut: Record<SmartList, string | null> = {
  [SmartList.Inbox]: "1",
  [SmartList.Today]: "2",
  [SmartList.Upcoming]: "3",
  [SmartList.Anytime]: "4",
  [SmartList.Someday]: "5",
  [SmartList.Logbook]: "6",
  [SmartList.Trash]: null,
};

export const SmartListTopGroup: SmartList[] = [
  SmartList.Inbox,
  SmartList.Today,
  SmartList.Upcoming,
  SmartList.Anytime,
  SmartList.Someday,
];

export const SmartListBottomGroup: SmartList[] = [
  SmartList.Logbook,
  SmartList.Trash,
];

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

// ---------------------------------------------------------------------------
// Legacy Task type (kept for backward compat with existing API/components)
// ---------------------------------------------------------------------------

export type Task = {
  id: string;
  title: string;
  description?: string | null;
  completed: boolean;
  priority?: number;
  due_date?: string | null;
  list_id?: string | null;
  user_id?: string;
  created_at: Date;
  updated_at?: Date;
};
