import type {
  DictationConfig,
  DictationConfigPatch,
  DictationConfigSnapshot,
  DictationConnectivity,
  DictationLocalModels,
  DictationPendingItem,
  DictationProgressEvent,
  DictationStats,
  FileIndexDiagnostics,
  FileIndexSettings,
  FileIndexSettingsPatch,
  FileIndexScanProgress,
  FocusActiveState,
  FocusBlocklist,
  EngineSettings,
  PackageItem,
  PackageSnapshot,
  PackageTrustStatus,
  SyncSnapshot,
  JsonRecord,
  JsonValue,
} from "../src/manager-api";

export type Input = JsonValue | undefined;
export type InputRecord = JsonRecord;
export const isString = (value: Input | undefined): value is string =>
  typeof value === "string";
export const isNumber = (value: Input | undefined): value is number =>
  typeof value === "number";
export const isBoolean = (value: Input | undefined): value is boolean =>
  typeof value === "boolean";

export const isObject = (value: Input | undefined): value is InputRecord =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);
const text = (value: Input | undefined, max: number, fallback = ""): string =>
  isString(value) && value.length <= max ? value : fallback;
const number = (
  value: Input | undefined,
  fallback = 0,
  max = Number.MAX_SAFE_INTEGER,
): number =>
  isNumber(value) && Number.isFinite(value)
    ? Math.max(0, Math.min(max, value))
    : fallback;
const flag = (value: Input | undefined): boolean => value === true;

export function normalizeEngineSettings(value: Input): EngineSettings {
  const root = isObject(value) ? value : {};
  const host = isObject(root.desktop_host) ? root.desktop_host : {};
  const tracker = isObject(root.usage_tracker) ? root.usage_tracker : {};
  return {
    desktop_host: {
      warm_timeout_seconds: host.warm_timeout_seconds === 0 ? 0 : 300,
    },
    usage_tracker: { enabled: tracker.enabled !== false },
  };
}

export function validateUsageTrackerSettingsPatch(
  value: Input,
): value is { enabled: boolean } {
  return (
    isObject(value) &&
    Object.keys(value).length === 1 &&
    isBoolean(value.enabled)
  );
}

const boundedList = (
  value: Input,
  maxItems: number,
  maxLength: number,
): string[] =>
  Array.isArray(value)
    ? value
        .filter(
          (item): item is string => isString(item) && item.length <= maxLength,
        )
        .slice(0, maxItems)
    : [];

export function validPackageId(value: Input): value is string {
  return isString(value) && /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(value);
}

export function validPackageVersion(value: Input): value is string {
  return (
    isString(value) &&
    /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(
      value,
    )
  );
}

function packageKind(value: Input): PackageItem["kind"] | null {
  return value === "app" || value === "source" || value === "bridge"
    ? value
    : null;
}

function normalizePackageItem(
  value: Input,
  catalog: boolean,
): PackageItem | null {
  if (
    !isObject(value) ||
    !validPackageId(value.id) ||
    !validPackageVersion(value.version)
  )
    return null;
  const kind = packageKind(value.kind);
  if (!kind) return null;
  return {
    id: value.id,
    name: text(value.name, 256, value.id),
    icon_path: text(value.icon_path, 2_048) || null,
    version: value.version,
    kind,
    publisher: text(value.publisher, 256, "Kosmos"),
    enabled: value.enabled === true,
    revoked: value.revoked === true,
    revocation_reason: text(value.revocation_reason, 256) || null,
    worker_state: text(value.worker_state, 64, catalog ? "catalog" : "stopped"),
    worker_health: text(
      value.worker_health,
      64,
      text(value.worker_state, 64, "unknown"),
    ),
    update_version: validPackageVersion(value.update_version)
      ? value.update_version
      : null,
    catalog,
  };
}

export function normalizePackageSnapshot(value: Input): PackageSnapshot {
  const raw = isObject(value) ? value : {};
  const installed = Array.isArray(raw.packages)
    ? raw.packages.flatMap((item: Input) => {
        const normalized = normalizePackageItem(item, false);
        return normalized ? [normalized] : [];
      })
    : [];
  const installedKeys = new Set(
    installed.map((item: PackageItem) => `${item.id}@${item.version}`),
  );
  const catalog = Array.isArray(raw.catalog)
    ? raw.catalog.flatMap((item: Input) => {
        const normalized = normalizePackageItem(item, true);
        return normalized &&
          !installedKeys.has(`${normalized.id}@${normalized.version}`)
          ? [normalized]
          : [];
      })
    : [];
  return {
    packages: installed,
    catalog,
    total: number(raw.total, installed.length, 100_000),
    truncated: raw.truncated === true,
  };
}

export function normalizePackageTrustStatus(value: Input): PackageTrustStatus {
  const root = isObject(value) ? value : {};
  const trust = isObject(root.trust) ? root.trust : {};
  const catalog = isObject(root.catalog) ? root.catalog : {};
  const configured = trust.configured === true;
  const expiresAt = text(catalog.expires_at, 128) || null;
  const expired =
    expiresAt !== null && Number.isNaN(Date.parse(expiresAt))
      ? true
      : expiresAt !== null && Date.parse(expiresAt) <= Date.now();
  const state: PackageTrustStatus["state"] = !configured
    ? "unavailable"
    : trust.fault_code || expired
      ? "stale"
      : "usable";
  const message =
    state === "usable"
      ? "Каталог доверен и доступен."
      : state === "stale"
        ? "Каталог устарел или временно недоступен."
        : "Доверие к каталогу не настроено.";
  return {
    state,
    message,
    configured,
    revoked_packages: number(trust.revoked_packages, 0, 100_000),
    catalog_sequence: number(
      trust.catalog_sequence,
      0,
      Number.MAX_SAFE_INTEGER,
    ),
    expires_at: expiresAt,
  };
}

function normalizeScanProgress(value: Input): FileIndexScanProgress {
  const raw = isObject(value) ? value : {};
  return {
    phase: text(raw.phase, 64, "idle"),
    root: isString(raw.root) && raw.root.length <= 4096 ? raw.root : null,
    roots_done: number(raw.roots_done, 0, 100_000),
    roots_total: number(raw.roots_total, 0, 100_000),
    files_seen: number(raw.files_seen, 0, 10_000_000_000),
    files_indexed: number(raw.files_indexed, 0, 10_000_000_000),
    message: text(raw.message, 256),
  };
}

export function validateFileIndexSettingsPatch(
  value: Input,
): FileIndexSettingsPatch | null {
  if (!isObject(value)) return null;
  const keys = Object.keys(value);
  const allowed = new Set([
    "enabled",
    "exclude_noisy_folders",
    "respect_gitignore",
    "include_hidden",
    "ntfs_accelerated",
  ]);
  if (!keys.length || keys.some((key) => !allowed.has(key))) return null;
  if (keys.some((key) => !isBoolean(value[key]))) return null;
  // SAFETY: keys and boolean values were validated immediately above.
  return value as FileIndexSettingsPatch;
}

export function validateFileIndexPath(
  value: Input,
  maxLength = 4096,
): value is string {
  if (!isString(value) || value.trim().length === 0 || value.length > maxLength)
    return false;
  const path = value.trim();
  return !path.startsWith("\\\\") && !path.startsWith("//");
}

export function normalizeFileIndexSettings(value: Input): FileIndexSettings {
  const raw =
    isObject(value) && isObject(value.settings)
      ? value.settings
      : isObject(value)
        ? value
        : {};
  const ntfsStatus = raw.ntfs_status;
  return {
    enabled: flag(raw.enabled),
    exclude_noisy_folders: flag(raw.exclude_noisy_folders),
    roots: boundedList(raw.roots, 128, 4096),
    ignore_patterns: boundedList(raw.ignore_patterns, 512, 512),
    respect_gitignore: flag(raw.respect_gitignore),
    include_hidden: flag(raw.include_hidden),
    ntfs_accelerated: flag(raw.ntfs_accelerated),
    scan_in_progress: flag(raw.scan_in_progress),
    scan_progress: normalizeScanProgress(raw.scan_progress),
    ntfs_status:
      ntfsStatus === "active" ||
      ntfsStatus === "fallback" ||
      ntfsStatus === "unavailable" ||
      ntfsStatus === "disabled"
        ? ntfsStatus
        : "unknown",
  };
}

export function normalizeFileIndexDiagnostics(
  value: Input,
): FileIndexDiagnostics {
  const raw =
    isObject(value) && isObject(value.diagnostics)
      ? value.diagnostics
      : isObject(value)
        ? value
        : {};
  const risk =
    raw.risk_level === "warning" || raw.risk_level === "danger"
      ? raw.risk_level
      : "ok";
  const riskReasons =
    risk === "danger"
      ? [
          "Индекс содержит потенциально слишком широкий корень или большой объём.",
        ]
      : risk === "warning"
        ? [
            "Индекс требует внимания по ограничению размера или полноте диагностики.",
          ]
        : [];
  return {
    db_size_bytes: number(raw.db_size_bytes, 0, Number.MAX_SAFE_INTEGER),
    wal_size_bytes: number(raw.wal_size_bytes, 0, Number.MAX_SAFE_INTEGER),
    total_size_bytes: number(raw.total_size_bytes, 0, Number.MAX_SAFE_INTEGER),
    scan_in_progress: flag(raw.scan_in_progress),
    scan_progress: normalizeScanProgress(raw.scan_progress),
    roots: boundedList(raw.roots, 128, 4096),
    roots_count: number(raw.roots_count, 0, 128),
    files_count: number(raw.files_count, 0, 10_000_000_000),
    risk_level: risk,
    risk_reasons: riskReasons,
    last_scan_ms: number(raw.last_scan_ms, 0, Number.MAX_SAFE_INTEGER),
  };
}

export function normalizeDictationConfig(
  value: Input,
): DictationConfigSnapshot {
  const root = isObject(value) ? value : {};
  const raw = isObject(root.config) ? root.config : root;
  const profile = isObject(raw.networkProfile) ? raw.networkProfile : {};
  const kind = profile.kind;
  const networkProfile: DictationConfig["networkProfile"] =
    kind === "custom_doh" && text(profile.url, 2048)
      ? { kind, url: text(profile.url, 2048) }
      : kind === "cloudflare_doh" || kind === "google_doh"
        ? { kind }
        : { kind: "system" };
  return {
    config: {
      microphoneDeviceId: isString(raw.microphoneDeviceId)
        ? text(raw.microphoneDeviceId, 512)
        : null,
      language: text(raw.language, 32, "ru"),
      triggerMode:
        raw.triggerMode === "push_to_talk" ? "push_to_talk" : "toggle",
      injectMode:
        raw.injectMode === "clipboard_only" ? "clipboard_only" : "auto_paste",
      hotkey: text(raw.hotkey, 128, "CTRL+SHIFT+SPACE"),
      duckAudioDuringRecording: flag(raw.duckAudioDuringRecording),
      localIdleUnloadMs: number(raw.localIdleUnloadMs, 300_000, 86_400_000),
      provider:
        raw.provider === "local" || raw.provider === "mock"
          ? raw.provider
          : "groq",
      providerEnabled: raw.providerEnabled !== false,
      model: text(raw.model, 128, "whisper-large-v3-turbo"),
      networkProfile,
    },
    hasApiKey: root.hasApiKey === true,
  };
}

export function validateDictationConfigPatch(
  value: Input,
): DictationConfigPatch | null {
  if (!isObject(value)) return null;
  const entries = Object.entries(value);
  if (
    !entries.length ||
    // SAFETY: the key is checked against the finite DictationConfigPatch key set.
    entries.some(
      ([key]) => !DICTATION_PATCH_KEYS.has(key as keyof DictationConfigPatch),
    )
  )
    return null;
  const patch: DictationConfigPatch = {};
  for (const [key, item] of entries) {
    if (key === "microphoneDeviceId" && (item === null || text(item, 512)))
      patch.microphoneDeviceId = item === null ? null : text(item, 512);
    else if (key === "language" && text(item, 32))
      patch.language = text(item, 32);
    else if (
      key === "triggerMode" &&
      (item === "toggle" || item === "push_to_talk")
    )
      patch.triggerMode = item;
    else if (
      key === "injectMode" &&
      (item === "auto_paste" || item === "clipboard_only")
    )
      patch.injectMode = item;
    else if (key === "hotkey" && text(item, 128))
      patch.hotkey = text(item, 128);
    else if (key === "duckAudioDuringRecording" && isBoolean(item))
      patch.duckAudioDuringRecording = item;
    else if (
      key === "localIdleUnloadMs" &&
      isNumber(item) &&
      Number.isInteger(item) &&
      item >= 0 &&
      item <= 86_400_000
    )
      patch.localIdleUnloadMs = item;
    else if (
      key === "provider" &&
      (item === "groq" || item === "local" || item === "mock")
    )
      patch.provider = item;
    else if (key === "providerEnabled" && isBoolean(item))
      patch.providerEnabled = item;
    else if (key === "model" && text(item, 128)) patch.model = text(item, 128);
    else if (key === "networkProfile" && isObject(item)) {
      const kind = item.kind;
      if (
        kind === "system" ||
        kind === "cloudflare_doh" ||
        kind === "google_doh"
      )
        patch.networkProfile = { kind };
      else if (
        kind === "custom_doh" &&
        text(item.url, 2048).startsWith("https://")
      )
        patch.networkProfile = { kind, url: text(item.url, 2048) };
      else return null;
    } else return null;
  }
  return patch;
}

export const DICTATION_PATCH_KEYS = new Set<keyof DictationConfigPatch>([
  "microphoneDeviceId",
  "language",
  "triggerMode",
  "injectMode",
  "hotkey",
  "duckAudioDuringRecording",
  "localIdleUnloadMs",
  "provider",
  "providerEnabled",
  "model",
  "networkProfile",
]);

export function validDictationId(value: Input): value is string {
  return isString(value) && /^[A-Za-z0-9._-]{1,128}$/.test(value);
}

export function normalizeDictationStats(value: Input): DictationStats {
  const raw = isObject(value) ? value : {};
  return {
    totalSeconds: number(raw.totalSeconds ?? raw.totalRecordSeconds),
    totalWords: number(raw.totalWords),
    savedSeconds: number(raw.savedSeconds ?? raw.timeSavedSeconds),
  };
}

export function normalizeDictationLocalModels(
  value: Input,
): DictationLocalModels {
  const raw = isObject(value) ? value : {};
  return Array.isArray(raw.models)
    ? raw.models.flatMap((item: Input) => {
        if (!isObject(item) || !validDictationId(item.id)) return [];
        return [
          {
            id: item.id,
            name: text(item.name, 128, item.id),
            description: text(item.description, 512),
            downloaded: flag(item.downloaded),
            selected: flag(item.selected),
          },
        ];
      })
    : [];
}

export function normalizeDictationPending(
  value: Input,
): DictationPendingItem[] {
  const raw = Array.isArray(value)
    ? value
    : isObject(value) && Array.isArray(value.items)
      ? value.items
      : [];
  return raw.flatMap((item: Input) => {
    if (!isObject(item) || !validDictationId(item.uuid)) return [];
    return [
      {
        uuid: item.uuid,
        createdAt: text(item.createdAt, 64),
        attempts: number(item.attempts),
        lastError: isString(item.lastError) ? text(item.lastError, 512) : null,
      },
    ];
  });
}

export function normalizeConnectivity(value: Input): DictationConnectivity {
  const raw = isObject(value) ? value : {};
  return {
    stages: Array.isArray(raw.stages)
      ? raw.stages.flatMap((item: Input) =>
          isObject(item) && text(item.name, 64)
            ? [
                {
                  name: text(item.name, 64),
                  ok: flag(item.ok),
                  ms: number(item.ms),
                  error: isString(item.error) ? text(item.error, 512) : null,
                },
              ]
            : [],
        )
      : [],
  };
}

export function normalizeDictationEvent(
  value: Input,
): DictationProgressEvent | null {
  const event = isObject(value) ? value : {};
  if (event.event === "dictation_capture_key")
    return {
      kind: "capture",
      accelerator: text(event.accelerator, 128) || null,
      cancelled: false,
    };
  if (event.event === "dictation_capture_cancelled")
    return { kind: "capture", accelerator: null, cancelled: true };
  if (!validDictationId(event.modelId)) return null;
  if (event.event === "dictation_local_model_download_progress")
    return {
      kind: "download",
      modelId: event.modelId,
      percent: isNumber(event.percent) ? number(event.percent, 0, 100) : null,
    };
  if (event.event === "dictation_local_model_download_complete")
    return { kind: "download_complete", modelId: event.modelId };
  if (event.event === "dictation_local_model_download_failed")
    return { kind: "download_failed", modelId: event.modelId };
  return null;
}

type PairingParams = { pairing_code: string };
type DeviceParams = { device_id: string };

export function toPairingParams(code: string): PairingParams {
  return { pairing_code: code.trim() };
}

export function toDeviceParams(peerId: string): DeviceParams {
  return { device_id: peerId.trim() };
}

export function validPairingCode(value: Input): value is string {
  return (
    isString(value) && value.trim().length >= 8 && value.trim().length <= 256
  );
}

export function normalizePairingCode(value: Input): string | null {
  if (!isString(value)) return null;
  const code = value.trim();
  return code && code.length <= 4096 ? code : null;
}

export function normalizeSyncSnapshot(value: Input): SyncSnapshot {
  const raw = isObject(value) ? value : {};
  const rawLocal = isObject(raw.local_device) ? raw.local_device : {};
  const transport =
    raw.transport === "iroh" ||
    raw.transport === "relay" ||
    raw.transport === "lan"
      ? raw.transport
      : "unknown";
  const validIso = (candidate: Input): string | null => {
    if (
      !isString(candidate) ||
      candidate.length > 128 ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?(?:Z|[+-]\d{2}:\d{2})$/.test(
        candidate,
      )
    )
      return null;
    const [, year, month, day, hour, minute, second] = candidate.match(
      /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})/,
    )!;
    const daysInMonth = new Date(
      Date.UTC(Number(year), Number(month), 0),
    ).getUTCDate();
    if (
      Number(month) < 1 ||
      Number(month) > 12 ||
      Number(day) < 1 ||
      Number(day) > daysInMonth ||
      Number(hour) > 23 ||
      Number(minute) > 59 ||
      Number(second) > 59
    )
      return null;
    const parsed = Date.parse(candidate);
    return Number.isNaN(parsed) ? null : new Date(parsed).toISOString();
  };
  const peers: SyncSnapshot["peers"] = Array.isArray(raw.peers)
    ? raw.peers.flatMap((peer: Input) => {
        if (!isObject(peer)) return [];
        const item = peer;
        const id = isString(item.device_id)
          ? item.device_id
          : isString(item.id)
            ? item.id
            : "";
        if (!id.trim() || id.length > 256) return [];
        const status =
          item.status === "online" || item.status === "offline"
            ? item.status
            : "unknown";
        // SAFETY: id/name/status are normalized to the peer contract before this cast.
        const normalized = {
          id,
          name:
            isString(item.device_name) && item.device_name.trim().length <= 256
              ? item.device_name.trim()
              : isString(item.name) && item.name.trim().length <= 256
                ? item.name.trim()
                : id,
          // SAFETY: status is restricted to the SyncSnapshot peer status literals above.
          status: status as SyncSnapshot["peers"][number]["status"],
        } as SyncSnapshot["peers"][number];
        const lastSeen = validIso(item.last_seen);
        normalized.last_seen = lastSeen;
        return [normalized];
      })
    : [];
  const running = raw.running === true;
  const boundedText = (candidate: Input): string | null => {
    if (!isString(candidate)) return null;
    const text = candidate.trim();
    return text.length > 0 && text.length <= 256 ? text : null;
  };
  const localId = boundedText(rawLocal.device_id) ?? boundedText(rawLocal.id);
  const localName =
    boundedText(rawLocal.device_name) ?? boundedText(rawLocal.name);
  const result: SyncSnapshot = {
    running,
    status: running ? "running" : "stopped",
    transport,
    local_device: localId ? { id: localId, name: localName ?? localId } : null,
    peers,
  };
  if (raw.pairing_available === true) result.pairing_available = true;
  if (raw.own_pairing_code_available === true)
    result.own_pairing_code_available = true;
  return result;
}

export function normalizeFocusBlocklists(value: Input): FocusBlocklist[] {
  const raw =
    isObject(value) && Array.isArray(value.blocklists) ? value.blocklists : [];
  return raw.flatMap((item: Input) => {
    if (!isObject(item) || !validFocusId(item.id) || !text(item.name, 256))
      return [];
    const domains = Array.isArray(item.domains)
      ? item.domains.filter(
          (entry: Input): entry is string =>
            isString(entry) && entry.length <= 512,
        )
      : [];
    return [
      {
        id: item.id,
        name: text(item.name, 256),
        domains,
        createdAt: text(item.createdAt, 128),
        preset: item.preset === true,
        icon: text(item.icon, 16),
        kind: item.kind === "raw" ? "raw" : "domains",
      },
    ];
  });
}

export function normalizeFocusActiveState(value: Input): FocusActiveState {
  const raw = isObject(value) ? value : {};
  return {
    active: raw.active === true,
    blocklist_id: validFocusId(raw.blocklist_id) ? raw.blocklist_id : null,
    started_at: text(raw.started_at, 128) || null,
  };
}

export function isActiveFocusBlocklist(value: Input, id: string): boolean {
  const state = normalizeFocusActiveState(value);
  return state.active && state.blocklist_id === id;
}

export function isValidFocusActiveState(value: Input): boolean {
  if (!isObject(value) || !isBoolean(value.active)) return false;
  if (
    value.blocklist_id !== undefined &&
    value.blocklist_id !== null &&
    !validFocusId(value.blocklist_id)
  )
    return false;
  return (
    value.started_at === undefined ||
    value.started_at === null ||
    text(value.started_at, 128) !== ""
  );
}

export function validFocusId(value: Input): value is string {
  return typeof value === "string" && /^[A-Za-z0-9._:-]{1,128}$/.test(value);
}

export function normalizeFocusServiceStatus(value: Input) {
  const raw = isObject(value) ? value : {};
  return {
    installed: raw.installed === true,
    running: raw.running === true,
    healthy: raw.healthy === true,
  };
}
