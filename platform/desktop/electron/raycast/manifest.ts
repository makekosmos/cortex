import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

export type RaycastCommandMode = "view" | "no-view" | "menu-bar";

export interface RaycastPreference {
  name: string;
  title?: string;
  type?: string;
  required?: boolean;
  default?: unknown;
}

export interface RaycastCommandManifest {
  name: string;
  title: string;
  subtitle?: string;
  description?: string;
  icon?: string;
  mode: RaycastCommandMode;
  keywords?: string[];
  preferences?: RaycastPreference[];
  arguments?: Array<Record<string, unknown>>;
}

export interface RaycastKosmosCommandConfig {
  entry?: string;
}

export interface RaycastKosmosConfig {
  permissions?: string[];
  windowEffect?: "acrylic" | "mica" | "none";
  minKosmosApiVersion?: string;
  commands?: Record<string, RaycastKosmosCommandConfig>;
}

export interface RaycastPackageManifest {
  name: string;
  title: string;
  version?: string;
  description?: string;
  author?: string;
  icon?: string;
  commands: RaycastCommandManifest[];
  preferences?: RaycastPreference[];
  kosmos?: RaycastKosmosConfig;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function optionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value : undefined;
}

function stringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const strings = value.filter((item): item is string => typeof item === "string");
  return strings.length > 0 ? strings : undefined;
}

function parseAuthor(value: unknown): string | undefined {
  if (typeof value === "string") return optionalString(value);
  const record = asRecord(value);
  return optionalString(record?.name);
}

function parsePreferences(value: unknown): RaycastPreference[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const preferences: RaycastPreference[] = [];
  for (const item of value) {
    const record = asRecord(item);
    const name = optionalString(record?.name);
    if (!record || !name) continue;
    preferences.push({
      name,
      title: optionalString(record.title),
      type: optionalString(record.type),
      required: typeof record.required === "boolean" ? record.required : undefined,
      default: record.default,
    });
  }
  return preferences.length > 0 ? preferences : undefined;
}

function parseCommandMode(value: unknown): RaycastCommandMode | null {
  if (value === undefined || value === null || value === "") return "view";
  if (value === "view" || value === "no-view" || value === "menu-bar") return value;
  return null;
}

function parseCommands(value: unknown): RaycastCommandManifest[] {
  if (!Array.isArray(value)) return [];
  const commands: RaycastCommandManifest[] = [];
  for (const item of value) {
    const record = asRecord(item);
    const name = optionalString(record?.name);
    const title = optionalString(record?.title);
    const mode = parseCommandMode(record?.mode);
    if (!record || !name || !title || !mode) continue;
    commands.push({
      name,
      title,
      subtitle: optionalString(record.subtitle),
      description: optionalString(record.description),
      icon: optionalString(record.icon),
      mode,
      keywords: stringArray(record.keywords),
      preferences: parsePreferences(record.preferences),
      arguments: Array.isArray(record.arguments)
        ? record.arguments.filter((arg): arg is Record<string, unknown> => asRecord(arg) !== null)
        : undefined,
    });
  }
  return commands;
}

function parseKosmosConfig(value: unknown): RaycastKosmosConfig | undefined {
  const record = asRecord(value);
  if (!record) return undefined;
  const commandsRecord = asRecord(record.commands);
  const commands: Record<string, RaycastKosmosCommandConfig> = {};
  if (commandsRecord) {
    for (const [name, config] of Object.entries(commandsRecord)) {
      const commandConfig = asRecord(config);
      const entry = optionalString(commandConfig?.entry);
      if (entry) commands[name] = { entry };
    }
  }
  const out: RaycastKosmosConfig = {
    permissions: stringArray(record.permissions),
    windowEffect:
      record.windowEffect === "acrylic" ||
      record.windowEffect === "mica" ||
      record.windowEffect === "none"
        ? record.windowEffect
        : undefined,
    minKosmosApiVersion: optionalString(record.minKosmosApiVersion),
    commands: Object.keys(commands).length > 0 ? commands : undefined,
  };
  return out.permissions || out.windowEffect || out.minKosmosApiVersion || out.commands
    ? out
    : undefined;
}

export function parseRaycastPackageManifest(value: unknown): RaycastPackageManifest | null {
  const record = asRecord(value);
  const name = optionalString(record?.name);
  if (!record || !name) return null;
  const commands = parseCommands(record.commands);
  if (commands.length === 0) return null;
  return {
    name,
    title: optionalString(record.title) ?? name,
    version: optionalString(record.version),
    description: optionalString(record.description),
    author: parseAuthor(record.author),
    icon: optionalString(record.icon),
    commands,
    preferences: parsePreferences(record.preferences),
    kosmos: parseKosmosConfig(record.kosmos),
  };
}

export function loadRaycastPackageManifest(extensionDir: string): RaycastPackageManifest | null {
  const manifestPath = path.join(extensionDir, "package.json");
  if (!existsSync(manifestPath)) return null;
  try {
    return parseRaycastPackageManifest(JSON.parse(readFileSync(manifestPath, "utf8")));
  } catch (error) {
    console.error(`[kepler-shell] Raycast package.json invalid: ${extensionDir}`, error);
    return null;
  }
}

export function resolveRaycastCommandEntry(
  extensionDir: string,
  manifest: RaycastPackageManifest,
  commandName: string,
): string | null {
  const configured = manifest.kosmos?.commands?.[commandName]?.entry;
  const candidates = configured
    ? [configured]
    : [
        `dist/${commandName}.mjs`,
        `dist/${commandName}.js`,
        `src/${commandName}.mjs`,
        `src/${commandName}.js`,
      ];
  const root = path.resolve(extensionDir);
  for (const candidate of candidates) {
    const resolved = path.resolve(root, candidate);
    const relative = path.relative(root, resolved);
    if (relative.startsWith("..") || path.isAbsolute(relative)) continue;
    if (existsSync(resolved)) return resolved;
  }
  return null;
}

export function raycastPreferenceDefaults(
  manifest: RaycastPackageManifest,
  commandName: string,
): Record<string, unknown> {
  const command = manifest.commands.find((item) => item.name === commandName);
  const values: Record<string, unknown> = {};
  for (const preference of [...(manifest.preferences ?? []), ...(command?.preferences ?? [])]) {
    if (preference.default !== undefined) values[preference.name] = preference.default;
  }
  return values;
}
