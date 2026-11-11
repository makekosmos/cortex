/**
 * The Manager's mirror of what the host pushes into the page. `manager.bridge`
 * fills it from the Engine (`/v1/health`, `/v1/info`, `/v1/rpc`); without the
 * host — `nsh dev`, `nsh test`, a browser — `demoState` keeps every page
 * readable instead of blank.
 */
import { signal } from "nsh";

/** The combination the Manager's event tap read, handed to the page that asked
 * for it: the widget shows nothing of its own while the keys are held, and the
 * page stores the result as a setting. */
export const hotkeyCaptured = signal("");
/** What the bridge and the poller report: a stopped Engine simply stops
 * answering, so that is `unreachable`, not a state of its own. */
export type EngineStatus = "running" | "starting" | "stopping" | "unreachable";

export type DeviceRow = {
  id: string;
  name: string;
  status: "online" | "offline";
  platform: string;
  current: boolean;
};

/** A device that Hello'd us and waits for «Принять / Отклонить» (KOS-369). */
export type PairingRequest = { id: string; name: string; platform: string; appVersion: string };

export type ModelRow = {
  id: string;
  name: string;
  size: string;
  downloaded: boolean;
  downloading: boolean;
  selected: boolean;
  percent: number;
};

/** One piece of the data folder. `detail` is the Engine's own split of it. */
export type StoragePart = {
  id: string;
  label: string;
  description: string;
  size: string;
  bytes: number;
  detail?: StoragePart[];
};

export type BackupRow = { id: string; size: string; modified: number };

/** One plugin, from the catalog or from the installed list: the page compares
 * the two by `id`, so both sides carry the same fields. */
export type PluginRow = {
  id: string;
  name: string;
  summary: string;
  /** The installed version; empty for a catalog entry, which has none yet. */
  version: string;
  running: boolean;
  /** Whether the plugin brought a program of its own (a builtin has none). */
  executable: boolean;
};

/** What a plugin setting holds: a key, a switch, a number, a choice — or
 * nothing yet. One owner claims it (`PluginSetting`), so the pages never pass
 * around "some JSON". */
export type SettingValue = string | number | boolean | null;

/** A setting as the plugin's manifest declares it. The Engine validates
 * against these, so a page can render any plugin without knowing it. */
export type PluginSetting =
  | { key: string; label: string; hint?: string; kind: { type: "text" }; value: SettingValue }
  | { key: string; label: string; hint?: string; kind: { type: "toggle" }; value: SettingValue }
  | {
      key: string;
      label: string;
      hint?: string;
      kind: { type: "number"; min: number; max: number };
      value: SettingValue;
    }
  | {
      key: string;
      label: string;
      hint?: string;
      kind: { type: "choice"; options: { value: SettingValue; label: string }[] };
      value: SettingValue;
    }
  | {
      key: string;
      label: string;
      hint?: string;
      kind: { type: "switch"; on: SettingValue; off: SettingValue };
      value: SettingValue;
    };

export type PluginManifest = {
  id: string;
  name: string;
  summary?: string;
  settings: PluginSetting[];
  /** Where the numbers on the page come from: the Engine reads this block,
   * calls the operation and hands back ready values. */
  stats?: { op: string; title?: string } | null;
};

/** One number on a plugin's page, already resolved by the Engine. */
export type PluginStat = {
  key: string;
  label: string;
  /** How to print it: a count, a count of words, or seconds as time. */
  format: string;
  value: number | string | null;
};

/** The numbers a plugin's page shows: a heading and its rows. */
export type PluginStats = { title: string; fields: PluginStat[] };

export type PluginsState = {
  /** What can be installed. `stale` says the Engine kept yesterday's copy. */
  catalog: { available: PluginRow[]; stale: boolean; url: string };
  installed: PluginRow[];
  /** The plugin whose page is open: its manifest and stored settings. */
  current: {
    id: string;
    manifest: PluginManifest | null;
    values: Record<string, SettingValue>;
    /** Statistics the Engine resolved for this page; absent when the plugin
     * declares none. */
    stats?: PluginStats | null;
  } | null;
};

export type ConnectorRow = {
  id: string;
  name: string;
  connected: boolean;
  kind: string;
  detail: string;
};

export type ManagerState = {
  engine: { status: EngineStatus; version: string; channel: string; port: number };
  client: { name: string };
  product: { bundleId?: string };
  sync: {
    running: boolean;
    /** The pairing code this computer showed, until the user hides it. */
    ownCode?: string;
    incoming: PairingRequest[];
    peers: DeviceRow[];
  };
  models: ModelRow[];
  keys: { hasApiKey: boolean; engine: string; language: string; model: string };
  usage: { todayMinutes: number; weekMinutes: number; top: { name: string; minutes: number }[] };
  /** `parts` is the Engine's own list of what the data folder holds. */
  storage: { dataDir: string; size: string; parts: StoragePart[] };
  backups: BackupRow[];
  connectors: ConnectorRow[];
  plugins: PluginsState;
  /** The global hotkey: the Manager's own event tap, and whether macOS lets it
   * watch the keyboard. The Engine cannot answer this — the grant belongs to
   * the program the person toggled in System Settings. */
  hotkey: { running: boolean; listenAccess: boolean; capturing: boolean; held: string[] };
  updates: {
    state: string;
    currentVersion: string;
    newVersion: string;
    channel: string;
    canInstall: boolean;
    percent: number;
  };
  settings: { autostart: boolean; telemetry: boolean };
};

export const demoState: ManagerState = {
  engine: {
    status: "running",
    version: "0.10.26",
    channel: "production",
    // Demo only: the real port comes from `engine.lock.json`.
    port: 47_321,
  },
  client: { name: "Этот компьютер" },
  product: { bundleId: "computer.mundus.app" },
  sync: {
    running: true,
    incoming: [],
    peers: [
      { id: "desktop", name: "Мастерская", status: "online", platform: "windows", current: false },
      { id: "phone", name: "Pixel 9", status: "offline", platform: "android", current: false },
    ],
  },
  models: [
    {
      id: "whisper-small",
      name: "Whisper Small",
      size: "466 МБ",
      downloaded: true,
      downloading: false,
      selected: true,
      percent: 0,
    },
    {
      id: "whisper-medium",
      name: "Whisper Medium",
      size: "1.5 ГБ",
      downloaded: false,
      downloading: false,
      selected: false,
      percent: 0,
    },
  ],
  keys: { hasApiKey: false, engine: "whisper.cpp", language: "ru", model: "whisper-small" },
  usage: {
    todayMinutes: 214,
    weekMinutes: 1_486,
    top: [
      { name: "Visual Studio Code", minutes: 96 },
      { name: "Figma", minutes: 41 },
      { name: "Ghostty", minutes: 38 },
    ],
  },
  storage: {
    dataDir: "C:\\Users\\you\\AppData\\Roaming\\Mundus",
    size: "1.8 ГБ",
    parts: [],
  },
  // The Engine's database is `ark.db`, so its backups carry that name.
  backups: [
    { id: "ark.db.backup-2026-11-07-201738", size: "3.9 МБ", modified: 1_794_082_659_398 },
    { id: "ark.db.backup-2026-11-04-151322", size: "475 КБ", modified: 1_793_805_202_952 },
  ],
  connectors: [
    {
      id: "greatfrontend",
      name: "GreatFrontend",
      connected: true,
      kind: "learning",
      detail: "63 задачи",
    },
    {
      id: "leetcode",
      name: "LeetCode",
      connected: false,
      kind: "learning",
      detail: "Не подключено",
    },
  ],
  hotkey: { running: true, listenAccess: true, capturing: false, held: [] },
  plugins: {
    catalog: {
      available: [
        {
          id: "com.kosmos.dictation",
          name: "Диктовка",
          summary: "Речь в текст по горячей клавише",
          version: "",
          running: false,
          executable: false,
        },
      ],
      stale: false,
      url: "https://raw.githubusercontent.com/makekosmos/cortex/main/plugins/catalog.json",
    },
    installed: [
      {
        id: "com.kosmos.dictation",
        name: "Диктовка",
        summary: "Речь в текст по горячей клавише",
        version: "0.5.2",
        running: false,
        executable: true,
      },
    ],
    current: null,
  },
  updates: {
    state: "available",
    currentVersion: "0.10.26",
    newVersion: "0.10.27",
    channel: "production",
    canInstall: false,
    percent: 0,
  },
  settings: { autostart: true, telemetry: false },
};

export function statusLabel(status: EngineStatus): string {
  switch (status) {
    case "running":
      return "Работает";
    case "starting":
      return "Запускается";
    case "stopping":
      return "Останавливается";
    default:
      return "Нет связи";
  }
}

export function minutes(value: number): string {
  const hours = Math.floor(value / 60);
  const rest = value % 60;
  if (!hours) return `${rest} мин`;
  return rest ? `${hours} ч ${rest} мин` : `${hours} ч`;
}

/**
 * The product version, as the Engine reports it. The Manager's crate version is
 * its own business and never stands in for it: with the Engine down the pages
 * show no version rather than a wrong one.
 */
export function productVersion(state: ManagerState): string {
  return state.engine.status === "running" ? state.engine.version : "";
}
