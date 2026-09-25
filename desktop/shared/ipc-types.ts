export interface StorageSummaryItem {
  id: string;
  label: string;
  path: string;
  bytes: number;
  exists: boolean;
  description?: string;
}

export interface StorageSummary {
  dataDir: string;
  userDataDir: string;
  totalBytes: number;
  items: StorageSummaryItem[];
}

export interface SearchResult {
  id: string;
  title: string;
  type_id: string;
  snippet?: string;
}

export interface ExportConverterInfo {
  converter_id: string;
  object_type: string;
  display_name: string;
  default_format: string;
  supported_formats: string[];
}

export interface ExportResult {
  files_written: string[];
  bytes: number;
  errors: string[];
}

export type NtfsStatus = "unknown" | "disabled" | "active" | "fallback" | "unavailable";

export interface FileIndexSettings {
  enabled: boolean;
  exclude_noisy_folders: boolean;
  roots: string[];
  ignore_patterns: string[];
  respect_gitignore: boolean;
  include_hidden: boolean;
  ntfs_accelerated: boolean;
  scan_in_progress: boolean;
  scan_progress: {
    phase: string;
    root: string | null;
    roots_done: number;
    roots_total: number;
    files_seen: number;
    files_indexed: number;
    message: string;
  };
  ntfs_status: NtfsStatus;
}

export interface FileIndexSettingsPatch {
  enabled?: boolean;
  exclude_noisy_folders?: boolean;
  respect_gitignore?: boolean;
  include_hidden?: boolean;
  ntfs_accelerated?: boolean;
}

type FileSearchRiskLevel = "ok" | "warning" | "danger";

interface FileIndexLastScanSnapshot {
  finished_at_unix_ms: number;
  duration_ms: number;
  indexed_file_count: number;
  roots_count: number;
  exclude_noisy_folders: boolean;
  respect_gitignore: boolean;
  include_hidden: boolean;
  ntfs_accelerated: boolean;
}

interface FileIndexDiagnosticsSnapshot {
  db_size_bytes: number;
  wal_size_bytes: number;
  total_size_bytes: number;
  scan_in_progress: boolean;
  scan_progress: FileIndexSettings["scan_progress"];
  roots: string[];
  roots_count: number;
  files_count: number;
  risk_level: FileSearchRiskLevel;
  risk_reasons: string[];
  last_scan_ms: number;
  last_scan: FileIndexLastScanSnapshot | null;
  search_count: number;
  like_search_count: number;
  query_len_histogram: Record<string, number>;
}

export interface FileSearchRootEstimate {
  path: string;
  scanned_dirs: number;
  scanned_files: number;
  ignored_or_skipped_files: number;
  indexable_text_files_count: number;
  indexable_text_bytes: number;
  metadata_only_media_files_count: number;
  metadata_only_other_files_count: number;
  estimated_indexed_entries_count: number;
  estimated_index_size_bytes: number;
  truncated: boolean;
  risk_level: FileSearchRiskLevel;
  risk_reasons: string[];
  limitations: string[];
}

export interface FileSearchRootWarning {
  path: string;
  risk_level: Exclude<FileSearchRiskLevel, "ok">;
  risk_reasons: string[];
}

export type FileSearchDiagnosticsReport = FileIndexDiagnosticsSnapshot;

/** Команда в launcher'е — единица того что пользователь может вызвать. */
export interface CommandRecord {
  id: string;
  title: string;
  /** Подпись справа: имя приложения / описание категории. */
  subtitle?: string;
  /** Группа для секций в UI: 'open' = запустить апку, 'action' = ручка апки. */
  category: "open" | "action";
  /** UI-классификация плашки. 'app' → правый лейбл «Приложение».
      'command' → «Команда · <appName>», 'file' → file-index hit.
      Если не указано — считается 'app'. */
  kind?: "app" | "command" | "file";
  /** Имя родительского приложения для command-плашек (Delphi / Kepler). */
  appName?: string;
  /** Опциональная иконка команды. Data URI (`data:image/png;base64,...`)
      для open-команд; undefined для action-команд. */
  icon?: string;
  /** Опциональный глобальный хоткей/accelerator, если команда имеет binding. */
  shortcut?: string;
}

type FocusSessionPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface FocusSessionPomodoroState {
  phase: FocusSessionPhase;
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  phaseEndsAtMs?: number | null;
  title?: string;
  tasks?: Array<{ id: string; title: string }>;
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

export interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

export interface FocusSessionTask {
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
  pomodoro: FocusSessionPomodoroState;
  focus: FocusActiveState;
  runningEntryId: string | null;
  focusError?: string | null;
}

export type { KeplerApi } from "./ipc-api-types";

export interface ReleaseBomIdentity {
  id: string;
  digest: string;
}

export type UpdateState =
  | { kind: "idle"; bom?: ReleaseBomIdentity }
  | { kind: "checking"; bom?: ReleaseBomIdentity }
  | { kind: "not-available"; checkedAt: number; bom?: ReleaseBomIdentity }
  | { kind: "available"; version: string; bom?: ReleaseBomIdentity }
  | { kind: "downloading"; version: string; percent: number; bom?: ReleaseBomIdentity }
  | { kind: "downloaded"; version: string; bom?: ReleaseBomIdentity }
  | { kind: "error"; message: string; bom?: ReleaseBomIdentity };
