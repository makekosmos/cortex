export type ManagerErrorCode =
  | "engine_unavailable"
  | "incompatible_api"
  | "validation"
  | "cancelled"
  | "transport"
  | "engine";
export type ManagerResult<T> =
  | { ok: true; data: T }
  | { ok: false; code: ManagerErrorCode; message: string };
export type JsonValue =
  string | number | boolean | null | JsonRecord | JsonValue[];
export interface JsonRecord {
  [key: string]: JsonValue;
}

export type EngineHealth = {
  ok: boolean;
  status: "ready";
  api_version: string;
};
export type EngineInfo = {
  ok: boolean;
  api_version: string;
  legacy_protocol_version: number;
  pid: number;
  ws_port: number;
  correlation_id: string;
};
export type DataTypeSummary = {
  id: string;
  type_id: string;
  type_version: "1.0.0";
  name: string;
  count: number;
  logical_bytes: number;
};
export type ObjectMetadata = {
  id: string;
  type_id: string;
  type_version: "1.0.0";
  title: string;
  fields?: JsonRecord;
  links?: Array<{ id: string; target_object_id: string; link_type: string }>;
  created_at?: string;
  updated_at?: string;
  excerpt?: string;
};
export type DataSummary = {
  collected_at?: string;
  types: DataTypeSummary[];
  count?: number;
  logical_bytes?: number;
  managed_storage_bytes: number;
};
export type ObjectPage = {
  items: ObjectMetadata[];
  next_cursor?: string;
  truncated?: boolean;
};
export type SyncPeer = {
  id: string;
  name: string;
  status: "online" | "offline" | "unknown";
  last_seen?: string | null;
};
export type SyncSnapshot = {
  running: boolean;
  status: "running" | "stopped";
  transport: "iroh" | "relay" | "lan" | "unknown";
  local_device: { id: string; name: string } | null;
  peers: SyncPeer[];
  pairing_available?: boolean;
  own_pairing_code_available?: boolean;
};
export type PackageItem = {
  id: string;
  name?: string;
  icon_path?: string | null;
  version: string;
  kind: "app" | "source" | "bridge";
  publisher: string;
  enabled?: boolean;
  revoked: boolean;
  revocation_reason?: string | null;
  worker_state?: string;
  worker_health?: string;
  update_version?: string | null;
  archive_size?: number;
  catalog?: boolean;
};
export type PackageSnapshot = {
  packages: PackageItem[];
  catalog: PackageItem[];
  total?: number;
  truncated?: boolean;
};
export type DesktopUpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "not-available"; checkedAt: number }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "downloaded"; version: string }
  | { kind: "error"; message: string };
export type StoreListing = {
  id: string;
  kind: "kosmos-package" | "integration" | "external-app";
  name: string;
  publisher?: string;
  publisher_tier?: "kosmos" | "verified" | "community";
  description?: string;
  categories?: string[];
  availability?: { platforms?: string[] };
  data_compatibility?: Array<{
    type: string;
    versions: string;
    roles: string[];
    via?: string;
    fidelity: string;
  }>;
  distribution?:
    | { package_id: string; version: string }
    | { official_url: string }
    | { package_id: string; version: string; connects_to: string };
  connects_to?: string;
  icon_url?: string;
  screenshots?: string[];
};
export type InstalledStoreItem = PackageItem & {
  effective_grants?: Array<{
    type: string;
    version?: string;
    roles: string[];
    fields_read: string[];
    fields_write: string[];
    relations_read: string[];
    relations_write: string[];
  }>;
};
export type StoreCatalogSnapshot = {
  state: "fresh" | "expired" | "unavailable";
  sequence?: number;
  issued_at?: string;
  expires_at?: string;
  listings: StoreListing[];
  installed: InstalledStoreItem[];
};
export type DevelopmentPackage = {
  id: string;
  name: string;
  version: string;
  publisher: string;
  icon_url: string;
};
export type BridgeConfig = {
  vault_root: string;
  selected_types: string[];
  editable_fields: string[];
  readonly_fields: string[];
};
export type BridgeWorkerStatus = {
  last_sync?: string;
  conflict_count: number;
  last_conflict_at?: string;
};
export type BridgeConfigStatus = {
  configured: boolean;
  config?: BridgeConfig;
  worker_state: string;
  worker_status?: BridgeWorkerStatus;
};
export type DiagnosticItem = {
  id?: string;
  name?: string;
  status?: string;
  state?: string;
  message?: string;
};
export type DiagnosticsSnapshot = {
  components: DiagnosticItem[];
  workers: DiagnosticItem[];
  legacy_gate: {
    api_v1_connections: number;
    legacy_connections: number;
    tracking_started_at?: string;
    api_v1_last_seen?: string;
    legacy_last_seen?: string;
    legacy_zero_since?: string;
    ready: boolean;
  };
};
export type EngineSettings = {
  desktop_host: { warm_timeout_seconds: 0 | 300 };
  usage_tracker: { enabled: boolean };
};
export type PairingCode = { code: string; expires_at?: string };

export type IntegrationProvider = {
  id:
    | "hevy"
    | "toggl"
    | "leetcode"
    | "codewars"
    | "greatfrontend"
    | "bigfrontend";
  label: string;
  credentialLabel: string;
  credentialUrl: string;
  hasCredential: boolean;
  settings: {
    intervalMinutes: number;
    syncOnStartup: boolean;
    lastAttemptAt?: string | null;
    lastSuccessAt?: string | null;
    lastError?: string | null;
    importedCount: number;
  };
};
export type IntegrationsSnapshot = {
  providers: IntegrationProvider[];
  bodyWeightKg?: number | null;
};

export type CrashReportMetadata = { name: string; size: number; mtime: string };
export type PackageTrustStatus = {
  state: "usable" | "stale" | "unavailable";
  message: string;
  configured: boolean;
  revoked_packages: number;
  catalog_sequence: number;
  expires_at: string | null;
};
export type DictationConfig = {
  microphoneDeviceId: string | null;
  language: string;
  triggerMode: "toggle" | "push_to_talk";
  injectMode: "auto_paste" | "clipboard_only";
  hotkey: string;
  duckAudioDuringRecording: boolean;
  localIdleUnloadMs: number;
  provider: "groq" | "mock" | "local";
  providerEnabled: boolean;
  model: string;
  networkProfile: DictationNetworkProfile;
};
export type DictationConfigSnapshot = {
  config: DictationConfig;
  hasApiKey: boolean;
};
export type DictationNetworkProfile =
  | { kind: "system" | "cloudflare_doh" | "google_doh" }
  | { kind: "custom_doh"; url: string };
export type DictationConfigPatch = Partial<
  Pick<
    DictationConfig,
    | "microphoneDeviceId"
    | "language"
    | "triggerMode"
    | "injectMode"
    | "hotkey"
    | "duckAudioDuringRecording"
    | "localIdleUnloadMs"
    | "provider"
    | "providerEnabled"
    | "model"
    | "networkProfile"
  >
>;
export type DictationStats = {
  totalSeconds: number;
  totalWords: number;
  savedSeconds: number;
};
export type DictationPendingItem = {
  uuid: string;
  createdAt: string;
  attempts: number;
  lastError: string | null;
};
export type DictationLocalModel = {
  id: string;
  name: string;
  description: string;
  downloaded: boolean;
  selected: boolean;
};
export type DictationLocalModels = DictationLocalModel[];
export type DictationConnectivity = {
  stages: Array<{
    name: string;
    ok: boolean;
    ms: number;
    error: string | null;
  }>;
};
export type DictationProgressEvent =
  | { kind: "capture"; accelerator: string | null; cancelled: boolean }
  | { kind: "download"; modelId: string; percent: number | null }
  | { kind: "download_complete" | "download_failed"; modelId: string };
export type FocusBlocklist = {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset: boolean;
  icon: string;
  kind: "domains" | "raw";
};
export type FocusActiveState = {
  active: boolean;
  blocklist_id: string | null;
  started_at: string | null;
};
export type FocusServiceStatus = {
  installed: boolean;
  running: boolean;
  healthy: boolean;
};
export type FocusServiceAction = { ok: boolean; error?: string };
export type FileIndexNtfsStatus =
  "active" | "fallback" | "unavailable" | "disabled" | "unknown";
export type FileIndexScanProgress = {
  phase: string;
  root: string | null;
  roots_done: number;
  roots_total: number;
  files_seen: number;
  files_indexed: number;
  message: string;
};
export type FileIndexSettings = {
  enabled: boolean;
  exclude_noisy_folders: boolean;
  roots: string[];
  ignore_patterns: string[];
  respect_gitignore: boolean;
  include_hidden: boolean;
  ntfs_accelerated: boolean;
  scan_in_progress: boolean;
  scan_progress: FileIndexScanProgress;
  ntfs_status: FileIndexNtfsStatus;
};
export type FileIndexSettingsPatch = Pick<
  FileIndexSettings,
  | "enabled"
  | "exclude_noisy_folders"
  | "respect_gitignore"
  | "include_hidden"
  | "ntfs_accelerated"
>;
export type FileIndexDiagnostics = {
  db_size_bytes: number;
  wal_size_bytes: number;
  total_size_bytes: number;
  scan_in_progress: boolean;
  scan_progress: FileIndexScanProgress;
  roots: string[];
  roots_count: number;
  files_count: number;
  risk_level: "ok" | "warning" | "danger";
  risk_reasons: string[];
  last_scan_ms: number;
};

export interface ManagerApi {
  getAppVersion(): Promise<ManagerResult<string>>;
  getDesktopUpdateState(): Promise<ManagerResult<DesktopUpdateState>>;
  checkDesktopUpdates(): Promise<ManagerResult<DesktopUpdateState>>;
  installDesktopUpdate(): Promise<ManagerResult<{ started: boolean }>>;
  getHealth(): Promise<ManagerResult<EngineHealth>>;
  getInfo(): Promise<ManagerResult<EngineInfo>>;
  getDataSummary(): Promise<ManagerResult<DataSummary>>;
  listObjectTypes(): Promise<ManagerResult<DataTypeSummary[]>>;
  listObjects(input: {
    type_id?: string;
    type_version?: "1.0.0";
    limit?: number;
    cursor?: string;
  }): Promise<ManagerResult<ObjectPage>>;
  searchObjects(input: { query: string }): Promise<ManagerResult<ObjectPage>>;
  getSyncSnapshot(): Promise<ManagerResult<SyncSnapshot>>;
  getPairingCode(): Promise<ManagerResult<PairingCode>>;
  connectWithPairingCode(input: {
    code: string;
  }): Promise<ManagerResult<unknown>>;
  getIntegrations(): Promise<ManagerResult<IntegrationsSnapshot>>;
  loginLeetCode(): Promise<ManagerResult<IntegrationsSnapshot>>;
  loginGreatFrontend(): Promise<ManagerResult<IntegrationsSnapshot>>;
  updateIntegrationSettings(input: {
    provider: IntegrationProvider["id"];
    intervalMinutes?: 0 | 15 | 60 | 360 | 1440;
    syncOnStartup?: boolean;
  }): Promise<ManagerResult<IntegrationsSnapshot>>;
  setIntegrationCredential(input: {
    provider: IntegrationProvider["id"];
    credential: string;
  }): Promise<ManagerResult<IntegrationsSnapshot>>;
  clearIntegrationCredential(input: {
    provider: IntegrationProvider["id"];
  }): Promise<ManagerResult<IntegrationsSnapshot>>;
  syncIntegrationNow(input: {
    provider: IntegrationProvider["id"];
  }): Promise<ManagerResult<IntegrationsSnapshot>>;
  disconnectPeer(input: { peer_id: string }): Promise<ManagerResult<unknown>>;
  getPackages(input?: {
    kind?: "app" | "source" | "bridge";
  }): Promise<ManagerResult<PackageSnapshot>>;
  getStoreCatalog(): Promise<ManagerResult<StoreCatalogSnapshot>>;
  refreshStoreCatalog(): Promise<ManagerResult<StoreCatalogSnapshot>>;
  openStoreExternal(input: {
    listing_id: string;
  }): Promise<ManagerResult<{ opened: boolean }>>;
  getPackageTrustStatus(): Promise<ManagerResult<unknown>>;
  refreshPackageCatalog(): Promise<ManagerResult<unknown>>;
  installPackage(input: {
    package_id: string;
    version: string;
  }): Promise<ManagerResult<unknown>>;
  openPackage(input: {
    package_id: string;
  }): Promise<ManagerResult<{ opened: boolean }>>;
  getDevelopmentPackages(): Promise<ManagerResult<DevelopmentPackage[]>>;
  openDevelopmentPackage(input: { package_id: string }): Promise<ManagerResult<{ opened: boolean }>>;
  setPackageEnabled(input: {
    package_id: string;
    version: string;
    enabled: boolean;
  }): Promise<ManagerResult<unknown>>;
  uninstallPackage(input: {
    package_id: string;
    version: string;
  }): Promise<ManagerResult<unknown>>;
  getBridgeConfig(input: {
    package_id: string;
    version: string;
  }): Promise<ManagerResult<BridgeConfigStatus>>;
  setBridgeConfig(input: {
    package_id: string;
    version: string;
    config: BridgeConfig;
  }): Promise<ManagerResult<BridgeConfigStatus>>;
  getDiagnosticsSnapshot(): Promise<ManagerResult<DiagnosticsSnapshot>>;
  getDiagnosticLogTail(input?: {
    component?: string;
    lines?: number;
  }): Promise<ManagerResult<{ entries: Array<JsonRecord> }>>;
  listCrashReports(): Promise<ManagerResult<CrashReportMetadata[]>>;
  clearCrashReports(): Promise<ManagerResult<{ removed: number }>>;
  openCrashReportsFolder(): Promise<ManagerResult<{ opened: boolean }>>;
  openLogsFolder(): Promise<ManagerResult<{ opened: boolean }>>;
  saveSupportBundle(): Promise<ManagerResult<{ saved: boolean }>>;
  getAutostart(): Promise<
    ManagerResult<{ enabled: boolean; available: boolean }>
  >;
  setAutostart(input: {
    enabled: boolean;
  }): Promise<ManagerResult<{ enabled: boolean; available: boolean }>>;
  getEngineSettings(): Promise<ManagerResult<EngineSettings>>;
  setWarmTimeout(input: {
    enabled: boolean;
  }): Promise<ManagerResult<EngineSettings>>;
  setUsageTracker(input: {
    enabled: boolean;
  }): Promise<ManagerResult<EngineSettings>>;
  getDictationConfig(): Promise<ManagerResult<DictationConfigSnapshot>>;
  updateDictationConfig(
    input: DictationConfigPatch,
  ): Promise<ManagerResult<DictationConfigSnapshot>>;
  getDictationStats(): Promise<ManagerResult<DictationStats>>;
  beginDictationHotkeyCapture(): Promise<ManagerResult<{ started: boolean }>>;
  endDictationHotkeyCapture(): Promise<ManagerResult<{ ended: boolean }>>;
  verifyDictationApiKey(input: {
    key: string;
  }): Promise<ManagerResult<{ valid: boolean; message: string }>>;
  setDictationApiKey(input: {
    key: string;
  }): Promise<ManagerResult<{ saved: boolean }>>;
  clearDictationApiKey(): Promise<ManagerResult<{ cleared: boolean }>>;
  testDictationConnectivity(): Promise<ManagerResult<DictationConnectivity>>;
  listDictationLocalModels(): Promise<ManagerResult<DictationLocalModels>>;
  downloadDictationLocalModel(input: {
    modelId: string;
    select?: boolean;
  }): Promise<ManagerResult<{ started: boolean; modelId: string }>>;
  useDictationLocalModel(input: {
    modelId: string;
  }): Promise<ManagerResult<DictationConfigSnapshot>>;
  deleteDictationLocalModel(input: {
    modelId: string;
  }): Promise<ManagerResult<DictationConfigSnapshot>>;
  listDictationPending(): Promise<ManagerResult<DictationPendingItem[]>>;
  retryDictation(input: {
    uuid: string;
  }): Promise<ManagerResult<{ started: boolean }>>;
  discardDictation(input: {
    uuid: string;
  }): Promise<ManagerResult<{ discarded: boolean }>>;
  retryAllDictation(): Promise<ManagerResult<{ started: number }>>;
  discardAllDictation(): Promise<ManagerResult<{ discarded: number }>>;
  onDictationEvent(
    listener: (event: DictationProgressEvent) => void,
  ): () => void;
  getFocusBlocklists(): Promise<ManagerResult<FocusBlocklist[]>>;
  getFocusActiveState(): Promise<ManagerResult<FocusActiveState>>;
  upsertFocusBlocklist(input: {
    id?: string;
    name: string;
    domains: string[];
    icon?: string;
    preset?: boolean;
  }): Promise<ManagerResult<FocusBlocklist>>;
  deleteFocusBlocklist(input: {
    id: string;
  }): Promise<ManagerResult<{ deleted: boolean }>>;
  getFocusServiceStatus(): Promise<ManagerResult<FocusServiceStatus>>;
  pingFocusService(): Promise<ManagerResult<{ healthy: boolean }>>;
  installFocusService(): Promise<ManagerResult<FocusServiceAction>>;
  uninstallFocusService(): Promise<ManagerResult<FocusServiceAction>>;
  startFocusService(): Promise<ManagerResult<FocusServiceAction>>;
  stopFocusService(): Promise<ManagerResult<FocusServiceAction>>;
  getFileIndexSettings(): Promise<ManagerResult<FileIndexSettings>>;
  setFileIndexSettings(
    input: FileIndexSettingsPatch,
  ): Promise<ManagerResult<FileIndexSettings>>;
  getFileIndexDiagnostics(): Promise<ManagerResult<FileIndexDiagnostics>>;
  addFileIndexRoot(input: {
    path: string;
  }): Promise<ManagerResult<FileIndexSettings>>;
  removeFileIndexRoot(input: {
    path: string;
  }): Promise<ManagerResult<FileIndexSettings>>;
  addFileIndexIgnore(input: {
    pattern: string;
  }): Promise<ManagerResult<FileIndexSettings>>;
  removeFileIndexIgnore(input: {
    pattern: string;
  }): Promise<ManagerResult<FileIndexSettings>>;
  rescanFileIndex(): Promise<ManagerResult<FileIndexSettings>>;
  clearFileIndexCache(): Promise<ManagerResult<FileIndexSettings>>;
  pickFileIndexRoot(): Promise<ManagerResult<string | null>>;
}

export const managerOperations = {
  getDataSummary: "manager.data.summary",
  listObjectTypes: "manager.data.types",
  listObjects: "manager.data.list",
  searchObjects: "manager.data.search",
  getSyncSnapshot: "get_sync_snapshot",
  getPairingCode: "get_own_iroh_ticket",
  connectWithPairingCode: "connect_with_pairing_code",
  getIntegrations: "integrations.list",
  updateIntegrationSettings: "integrations.update_settings",
  setIntegrationCredential: "integrations.set_credential",
  clearIntegrationCredential: "integrations.clear_credential",
  syncIntegrationNow: "integrations.sync_now",
  disconnectPeer: "disconnect_peer",
  getPackages: "packages.list",
  getStoreCatalog: "store.catalog",
  refreshStoreCatalog: "store.refresh",
  openStoreExternal: "store.external_url",
  getPackageTrustStatus: "packages.trust_status",
  refreshPackageCatalog: "packages.refresh_catalog",
  installPackage: "packages.install",
  setPackageEnabled: "packages.set_enabled",
  uninstallPackage: "packages.uninstall",
  getBridgeConfig: "packages.bridge_config",
  setBridgeConfig: "packages.bridge_config_set",
  getDiagnosticsSnapshot: "manager.diagnostics.snapshot",
  getDiagnosticLogTail: "manager.diagnostics.log_tail",
  createSupportBundle: "manager.diagnostics.support_bundle.create",
  saveSupportBundle: "manager.diagnostics.support_bundle.save",
  cancelSupportBundle: "manager.diagnostics.support_bundle.cancel",
  getEngineSettings: "engine.settings.get",
  setWarmTimeout: "engine.settings.set",
  setUsageTracker: "engine.settings.set",
  getDictationConfig: "dictation.get_config",
  updateDictationConfig: "dictation.update_config",
  getDictationStats: "dictation.get_stats",
  beginDictationHotkeyCapture: "dictation.begin_hotkey_capture",
  endDictationHotkeyCapture: "dictation.end_hotkey_capture",
  verifyDictationApiKey: "dictation.verify_api_key",
  setDictationApiKey: "dictation.set_api_key",
  clearDictationApiKey: "dictation.clear_api_key",
  testDictationConnectivity: "dictation.test_connectivity",
  listDictationLocalModels: "dictation.list_local_models",
  downloadDictationLocalModel: "dictation.download_local_model",
  useDictationLocalModel: "dictation.use_local_model",
  deleteDictationLocalModel: "dictation.delete_local_model",
  listDictationPending: "dictation.list_pending",
  retryDictation: "dictation.retry",
  discardDictation: "dictation.discard",
  retryAllDictation: "dictation.retry_all",
  discardAllDictation: "dictation.discard_all",
  getFocusBlocklists: "focus.list_blocklists",
  getFocusActiveState: "focus.get_active_state",
  upsertFocusBlocklist: "focus.upsert_blocklist",
  deleteFocusBlocklist: "focus.delete_blocklist",
  getFileIndexSettings: "file_index.settings_get",
  setFileIndexSettings: "file_index.settings_set",
  getFileIndexDiagnostics: "file_index.diagnostics",
  addFileIndexRoot: "file_index.scope_add",
  removeFileIndexRoot: "file_index.scope_remove",
  addFileIndexIgnore: "file_index.ignore_add",
  removeFileIndexIgnore: "file_index.ignore_remove",
  rescanFileIndex: "file_index.rescan",
  clearFileIndexCache: "file_index.clear_cache",
} as const;
