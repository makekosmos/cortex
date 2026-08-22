import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import { resolveInstance } from "./instance";

interface KeplerShellSettings {
  hotkey?: string;
  showTrayIcon?: boolean;
  /** Сколько минут хранить позицию в лаунчере. 0 = всегда ресетим. */
  launcherStateTtlMinutes?: number;
}

const DEFAULT_HOTKEY_PROD = process.platform === "darwin" ? "Command+Space" : "Alt+Space";
export const DEFAULT_HOTKEY = resolveInstance().hotkey ?? DEFAULT_HOTKEY_PROD;

const MODIFIER_ALIASES: Record<string, string> = {
  cmd: "Command",
  command: "Command",
  meta: process.platform === "darwin" ? "Command" : "Super",
  super: process.platform === "darwin" ? "Command" : "Super",
  win: "Super",
  windows: "Super",
  option: "Alt",
  alt: "Alt",
  ctrl: "Control",
  control: "Control",
  shift: "Shift",
};

const MODIFIER_ORDER = ["Command", "Control", "Alt", "Shift", "Super"];
const DEFAULT_LAUNCHER_STATE_TTL_MIN = 5;

function settingsFilePath(): string {
  return path.join(keplerDataDir(), "kepler-shell-settings.json");
}

function normalizeHotkeyPart(part: string): string {
  const trimmed = part.trim();
  if (!trimmed) return "";
  const alias = MODIFIER_ALIASES[trimmed.toLowerCase()];
  if (alias) return alias;
  if (trimmed.length === 1) return trimmed.toUpperCase();
  if (trimmed.toLowerCase() === "space") return "Space";
  if (trimmed.toLowerCase() === "escape") return "Escape";
  return trimmed;
}

export function normalizeHotkeyAccelerator(value: string): string {
  const parts = String(value || "")
    .split("+")
    .map(normalizeHotkeyPart)
    .filter(Boolean);
  if (parts.length === 0) return "";

  const modifiers = new Set<string>();
  let main = "";
  for (const part of parts) {
    if (MODIFIER_ORDER.includes(part)) {
      modifiers.add(part);
    } else if (!main) {
      main = part;
    }
  }
  if (!main) return "";
  return [...MODIFIER_ORDER.filter((part) => modifiers.has(part)), main].join("+");
}

export function readSettings(): KeplerShellSettings {
  try {
    const file = settingsFilePath();
    if (!existsSync(file)) return {};
    return JSON.parse(readFileSync(file, "utf8")) as KeplerShellSettings;
  } catch {
    return {};
  }
}

export function writeSettings(patch: Partial<KeplerShellSettings>): void {
  const current = readSettings();
  const next: KeplerShellSettings = { ...current, ...patch };
  const target = settingsFilePath();
  const tmp = target + ".tmp";
  try {
    writeFileSync(tmp, JSON.stringify(next, null, 2), "utf8");
    renameSync(tmp, target);
  } catch (e) {
    console.error("[kepler-shell] settings write failed:", e);
  }
}

export function isTrayIconEnabled(): boolean {
  return readSettings().showTrayIcon !== false;
}

export function setTrayIconEnabled(enabled: boolean): void {
  writeSettings({ showTrayIcon: !!enabled });
}

export function getStoredHotkey(): string {
  const raw = readSettings().hotkey ?? DEFAULT_HOTKEY;
  const normalized = normalizeHotkeyAccelerator(raw);
  if (normalized && normalized !== raw) {
    writeSettings({ hotkey: normalized });
  }
  return normalized || DEFAULT_HOTKEY;
}

export function setStoredHotkey(value: string): void {
  const normalized = normalizeHotkeyAccelerator(value);
  writeSettings({ hotkey: normalized || DEFAULT_HOTKEY });
}

export function getLauncherStateTtlMinutes(): number {
  const v = readSettings().launcherStateTtlMinutes;
  if (typeof v !== "number" || Number.isNaN(v) || v < 0) {
    return DEFAULT_LAUNCHER_STATE_TTL_MIN;
  }
  return Math.floor(v);
}
