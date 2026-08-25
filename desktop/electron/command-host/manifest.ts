import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

type JsonPrimitive = string | number | boolean | null;
interface JsonRecord {
  [key: string]: JsonValue;
}
type JsonValue = JsonPrimitive | JsonValue[] | JsonRecord;
type CommandMode = "view" | "no-view" | "menu-bar";

interface CommandPreference {
  name: string;
  title?: string;
  type?: string;
  required?: boolean;
  default?: JsonValue;
}

interface CommandManifestEntry {
  name: string;
  title: string;
  subtitle?: string;
  description?: string;
  icon?: string;
  mode: CommandMode;
  keywords?: string[];
  preferences?: CommandPreference[];
  arguments?: JsonRecord[];
}

interface CommandKosmosCommandConfig {
  entry?: string;
}

interface CommandKosmosConfig {
  permissions?: string[];
  windowEffect?: "acrylic" | "mica" | "none";
  minKosmosApiVersion?: string;
  commands?: Record<string, CommandKosmosCommandConfig>;
}

export interface CommandPackageManifest {
  name: string;
  title: string;
  version?: string;
  description?: string;
  author?: string;
  icon?: string;
  commands: CommandManifestEntry[];
  preferences?: CommandPreference[];
  kosmos?: CommandKosmosConfig;
}

function isRecord(value: JsonValue | undefined): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function asRecord(value: JsonValue | undefined): JsonRecord | null {
  return isRecord(value) ? value : null;
}

function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}

function isBoolean(value: JsonValue | undefined): value is boolean {
  return typeof value === "boolean";
}

function optionalString(value: JsonValue | undefined): string | undefined {
  return isString(value) && value.trim().length > 0 ? value : undefined;
}

function stringArray(value: JsonValue | undefined): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const strings = value.filter((item): item is string => typeof item === "string");
  return strings.length > 0 ? strings : undefined;
}

function parseAuthor(value: JsonValue | undefined): string | undefined {
  if (isString(value)) return optionalString(value);
  const record = asRecord(value);
  return optionalString(record?.name);
}

function parsePreferences(value: JsonValue | undefined): CommandPreference[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const preferences: CommandPreference[] = [];
  for (const item of value) {
    const record = asRecord(item);
    const name = optionalString(record?.name);
    if (!record || !name) continue;
    preferences.push({
      name,
      title: optionalString(record.title),
      type: optionalString(record.type),
      required: isBoolean(record.required) ? record.required : undefined,
      default: record.default,
    });
  }
  return preferences.length > 0 ? preferences : undefined;
}

function parseCommandMode(value: JsonValue | undefined): CommandMode | null {
  if (value === undefined || value === null || value === "") return "view";
  if (value === "view" || value === "no-view" || value === "menu-bar") return value;
  return null;
}

function parseCommands(value: JsonValue | undefined): CommandManifestEntry[] {
  if (!Array.isArray(value)) return [];
  const commands: CommandManifestEntry[] = [];
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
        ? record.arguments.filter((arg): arg is JsonRecord => asRecord(arg) !== null)
        : undefined,
    });
  }
  return commands;
}

function parseKosmosConfig(value: JsonValue | undefined): CommandKosmosConfig | undefined {
  const record = asRecord(value);
  if (!record) return undefined;
  const commandsRecord = asRecord(record.commands);
  const commands: Record<string, CommandKosmosCommandConfig> = {};
  if (commandsRecord) {
    for (const [name, config] of Object.entries(commandsRecord)) {
      const commandConfig = asRecord(config);
      const entry = optionalString(commandConfig?.entry);
      if (entry) commands[name] = { entry };
    }
  }
  const out: CommandKosmosConfig = {
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

export function parseCommandPackageManifest(value: JsonValue): CommandPackageManifest | null {
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

export function loadCommandPackageManifest(extensionDir: string): CommandPackageManifest | null {
  const manifestPath = path.join(extensionDir, "package.json");
  if (!existsSync(manifestPath)) return null;
  try {
    // SAFETY: package.json is parsed into the closed JSON value grammar before validation.
    return parseCommandPackageManifest(JSON.parse(readFileSync(manifestPath, "utf8")) as JsonValue);
  } catch (error) {
    console.error(`[kepler-shell] Command package.json invalid: ${extensionDir}`, error);
    return null;
  }
}

export function resolveCommandEntry(
  extensionDir: string,
  manifest: CommandPackageManifest,
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

export function commandPreferenceDefaults(
  manifest: CommandPackageManifest,
  commandName: string,
): JsonRecord {
  const command = manifest.commands.find((item) => item.name === commandName);
  const values: JsonRecord = {};
  for (const preference of [...(manifest.preferences ?? []), ...(command?.preferences ?? [])]) {
    if (preference.default !== undefined) values[preference.name] = preference.default;
  }
  return values;
}
