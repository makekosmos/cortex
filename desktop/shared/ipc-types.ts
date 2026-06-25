// Контракт IPC между main и renderer для Kepler launcher.
//
// Renderer вызывает методы через `window.kepler.*` (см. preload.ts).
// Main process реализует handlers в electron/main.ts через ipcMain.handle().

/**
 * Фича «История буфера обмена» ЗАМОРОЖЕНА (2026-06-06) и скрыта во всех сборках.
 * Единый рубильник: им загейтлены запуск поллинга/IPC (main), команда лаунчера
 * и вкладка настроек (renderer). Код намеренно оставлен в репо для будущей
 * доработки — подробности и причина в `docs-site/concepts/clipboard-history.md`.
 * Чтобы вернуть фичу — поставить `true` (и доделать производительность поллинга).
 */
export const CLIPBOARD_HISTORY_ENABLED = false;

export interface BackendStatus {
  running: boolean;
  pid?: number;
  wsPort?: number;
  lockFilePath: string;
}

export type SyncPeerStatus = "online" | "offline";
export type SyncPeerDeviceKind = "desktop" | "laptop" | "phone" | "unknown";

export interface SyncPeerInfo {
  deviceId: string;
  deviceName: string;
  lastSeen: string | null;
  status: SyncPeerStatus;
  deviceKind: SyncPeerDeviceKind;
}

export interface SyncStatusSnapshot {
  running: boolean;
  transport: "iroh" | "relay" | "lan" | "unknown";
  pairingAvailable: boolean;
  ownPairingCodeAvailable: boolean;
  peers: SyncPeerInfo[];
  localDevice?: { deviceId: string; deviceName: string } | null;
}

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

/** Marketplace catalog entry — соответствует `CatalogExtension` из
    `platform/desktop/electron/extension-marketplace.ts`. */
export interface MarketplaceExtension {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  keplerApiVersion: string;
  iconUrl: string | null;
  downloadUrl: string;
  sha256: string | null;
  size: number | null;
}

export interface MarketplaceCatalog {
  schemaVersion: number;
  updatedAt: string;
  extensions: MarketplaceExtension[];
}

/** Один зарегистрированный converter в backend export registry. Phase 7. */
export interface ExportConverterInfo {
  converter_id: string;
  object_type: string;
  display_name: string;
  default_format: string;
  supported_formats: string[];
}

/** Результат `export.run` — что записано на диск и какие были ошибки. */
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

export interface InstalledExtensionInfo {
  id: string;
  name: string;
  kind: "vue" | "static" | "native" | null;
  version: string | null;
  description: string | null;
  author: string | null;
  iconDataUri: string | null;
  backupCount: number;
  backupTimestamps: string[];
  /** Источник кода extension'а:
   * - `"installed"` — user-installed в `<dataDir>/extensions/<id>/` (production flow)
   * - `"dev"` — repo dev tree (`<repoRoot>/extensions/<id>/`); auto-detect'ится
   *   когда Kepler shell запущен из репо. UI скрывает revert/uninstall кнопки
   *   для dev-source extensions (они tracked git'ом, не Kepler'ом). */
  source: "installed" | "dev";
}

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
      для open-команд extension'ов; undefined для action-команд. */
  icon?: string;
  /** Опциональный глобальный хоткей/accelerator, если команда имеет binding. */
  shortcut?: string;
}

export interface ClipboardHistoryItem {
  id: string;
  kind: "text" | "image" | "link" | "color" | "file";
  text: string;
  preview: string;
  createdAt: number;
  updatedAt: number;
  charCount: number;
  pinned: boolean;
  searchText: string;
  source?: string;
  sourceIcon?: string;
  imageDataUrl?: string;
  width?: number;
  height?: number;
  url?: string;
  color?: string;
  filePath?: string;
  fileName?: string;
  mimeType?: string;
  storageBytes?: number;
}

export interface ClipboardHistorySettings {
  retentionDays: number;
  maxBytes: number;
}

export interface ClipboardHistorySettingsPatch {
  retentionDays?: number;
  maxBytes?: number;
}

export interface ClipboardHistoryStats {
  itemCount: number;
  pinnedCount: number;
  storageBytes: number;
  oldestItemAt: number | null;
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
}

export type { KeplerApi } from "./ipc-api-types";

/** autoUpdater state machine. См. platform/desktop/electron/autoupdater-host.ts. */
export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "not-available"; checkedAt: number }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "downloaded"; version: string }
  | { kind: "error"; message: string };
