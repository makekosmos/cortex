/**
 * The Manager's mirror of what the host pushes into the page. `manager.bridge`
 * fills it from the Engine (`/v1/health`, `/v1/info`, `/v1/rpc`); without the
 * host — `nsh dev`, `nsh test`, a browser — `demoState` keeps every page
 * readable instead of blank.
 */
export type EngineStatus = "running" | "starting" | "stopping" | "stopped" | "unreachable";

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

export type BackupRow = { id: string; size: string; modified: number };

export type ConnectorRow = {
  id: string;
  name: string;
  connected: boolean;
  kind: string;
  detail: string;
};

export type ManagerState = {
  engine: {
    status: EngineStatus;
    version: string;
    channel: string;
    port: number;
    uptimeMinutes: number;
  };
  client: { name: string; remoteControl: boolean };
  product: { channel: string; bundleId?: string };
  sync: {
    running: boolean;
    pairingAvailable: boolean;
    ownCodeAvailable: boolean;
    ownCode?: string;
    incoming: PairingRequest[];
    peers: DeviceRow[];
  };
  devices: DeviceRow[];
  apps: { id: string; name: string; version: string; installed: boolean }[];
  models: ModelRow[];
  keys: { hasApiKey: boolean; engine: string; language: string; model: string };
  usage: { todayMinutes: number; weekMinutes: number; top: { name: string; minutes: number }[] };
  storage: { dataDir: string; size: string; lastBackup: string };
  backups: BackupRow[];
  connectors: ConnectorRow[];
  updates: {
    state: string;
    currentVersion: string;
    newVersion: string;
    channel: string;
    canInstall: boolean;
    percent: number;
    checkedAt: string;
  };
  settings: { autostart: boolean; telemetry: boolean; language: string; localOnly: boolean };
  appearance: {
    mode: string;
    light: string;
    dark: string;
    followApps: boolean;
    accentSource: string;
    accentColor: string;
  };
};

export const demoState: ManagerState = {
  engine: {
    status: "running",
    version: "0.10.26",
    channel: "production",
    port: 47_321,
    uptimeMinutes: 186,
  },
  client: { name: "Кирилл — ноутбук", remoteControl: false },
  product: { channel: "production", bundleId: "com.kazui.mundus.manager" },
  sync: {
    running: true,
    pairingAvailable: true,
    ownCodeAvailable: true,
    incoming: [],
    peers: [
      { id: "desktop", name: "Мастерская", status: "online", platform: "windows", current: false },
      { id: "phone", name: "Pixel 9", status: "offline", platform: "android", current: false },
    ],
  },
  devices: [
    { id: "this", name: "Кирилл — ноутбук", status: "online", platform: "windows", current: true },
    { id: "desktop", name: "Мастерская", status: "online", platform: "windows", current: false },
    { id: "phone", name: "Pixel 9", status: "offline", platform: "android", current: false },
  ],
  // The Engine keeps no installed-app catalog on this path, so the page has
  // nothing to show: see the empty state in `pages/library.tsx`.
  apps: [],
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
    dataDir: "C:\\Users\\kirill\\AppData\\Roaming\\Mundus",
    size: "1.8 ГБ",
    lastBackup: "сегодня, 09:14",
  },
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
  updates: {
    state: "available",
    currentVersion: "0.10.26",
    newVersion: "0.10.27",
    channel: "production",
    canInstall: false,
    percent: 0,
    checkedAt: "",
  },
  settings: { autostart: true, telemetry: false, language: "Русский", localOnly: true },
  appearance: {
    mode: "dark",
    light: "merino-light",
    dark: "cobalt-foundry",
    followApps: false,
    accentSource: "theme",
    accentColor: "",
  },
};

export function statusLabel(status: EngineStatus): string {
  switch (status) {
    case "running":
      return "Работает";
    case "starting":
      return "Запускается";
    case "stopping":
      return "Останавливается";
    case "stopped":
      return "Остановлен";
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
