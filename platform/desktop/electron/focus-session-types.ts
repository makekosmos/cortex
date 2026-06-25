export type PomodoroPhase = "idle" | "work" | "shortBreak" | "longBreak";

export interface PomodoroState {
  phase: PomodoroPhase;
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  phaseEndsAtMs?: number | null;
  title?: string;
  tasks?: Array<{ id: string; title: string }>;
}

export interface ArkObjectLike {
  id: string;
  typeId?: string;
  type_id?: string;
  title?: string | null;
  contentJson?: unknown;
  content_json?: unknown;
  propsJson?: Record<string, unknown>;
  props_json?: Record<string, unknown>;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  deletedAt?: string | null;
  deleted_at?: string | null;
}

export interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

export interface FocusActiveState {
  active: boolean;
  blocklist_id?: string | null;
  blocked_app_ids?: string[];
  blocked_apps?: FocusBlockedApp[];
  started_at?: string | null;
}

export interface FocusBlockedApp {
  id: string;
  name: string;
  icon?: string | null;
  exec_path?: string | null;
}

export interface DelphiTask {
  id: string;
  title: string;
  status?: string | null;
}

export interface StartFocusSessionInput {
  title: string;
  durationMin: number;
  taskId?: string | null;
  taskTitle?: string | null;
  mode?: "block" | "allow";
  categoryIds?: string[];
  blocklistId?: string | null;
  blockedAppIds?: string[];
  blockedApps?: FocusBlockedApp[];
}

export interface FocusSessionSnapshot {
  pomodoro: PomodoroState;
  focus: FocusActiveState;
  runningEntryId: string | null;
}
