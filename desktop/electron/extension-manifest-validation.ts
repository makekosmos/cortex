import path from "node:path";
import { isBoolean, isRecord, isString } from "../src/shared/runtimeGuards";
import type { JsonRecord } from "../src/shared/runtimeGuards";

export interface ExtensionManifest {
  id: string;
  appId?: string;
  name: string;
  kind?: string;
  version?: string;
  description?: string;
  author?: string;
  icon?: string;
  entryHtml?: string;
  preload?: string;
  keplerApiVersion?: string;
  keepAliveInBackground?: boolean;
  native?: {
    executable: string;
    devExecutable?: string;
    cargoPackage?: string;
    args?: string[];
    singleInstance?: boolean;
  };
}

type ExtensionManifestInput = JsonRecord | null;

function optionalString(manifest: JsonRecord, field: string): string | undefined {
  const value = manifest[field];
  if (value !== undefined && !isString(value)) {
    throw new Error(`manifest.${field} должен быть строкой`);
  }
  return isString(value) ? value : undefined;
}

function safeRelativePath(value: string, field: string): string {
  const normalized = value.replaceAll("\\", "/");
  if (
    value.length === 0 ||
    value.includes("\0") ||
    path.posix.isAbsolute(normalized) ||
    path.win32.isAbsolute(value) ||
    /^[a-zA-Z]:/.test(normalized) ||
    normalized.split("/").some((part) => part === "..")
  ) {
    throw new Error(`manifest.${field} должен быть безопасным относительным путём`);
  }
  return normalized;
}

function optionalPath(manifest: JsonRecord, field: string): string | undefined {
  const value = optionalString(manifest, field);
  return value === undefined ? undefined : safeRelativePath(value, field);
}

/** Validates the manifest before any optional value is used to derive a path. */
export function validateExtensionManifest(value: ExtensionManifestInput): ExtensionManifest {
  if (!isRecord(value)) {
    throw new Error("manifest.json должен содержать объект");
  }
  const manifest = value;
  if (!isString(manifest.id) || !manifest.id) {
    throw new Error("manifest.id обязателен и должен быть строкой");
  }
  if (!/^\w[\w.-]*$/.test(manifest.id)) {
    throw new Error(`manifest.id невалиден: ${manifest.id}`);
  }
  if (!isString(manifest.name) || !manifest.name.trim()) {
    throw new Error("manifest.name обязателен и должен быть строкой");
  }

  const appId = optionalString(manifest, "appId");
  const version = optionalString(manifest, "version");
  const description = optionalString(manifest, "description");
  const author = optionalString(manifest, "author");
  const kind = optionalString(manifest, "kind");
  const keplerApiVersion = optionalString(manifest, "keplerApiVersion");
  const icon = optionalPath(manifest, "icon");
  const entryHtml = optionalPath(manifest, "entryHtml");
  const preload = optionalPath(manifest, "preload");

  const keepAlive = manifest.keepAliveInBackground;
  if (keepAlive !== undefined && !isBoolean(keepAlive)) {
    throw new Error("manifest.keepAliveInBackground должен быть boolean");
  }

  const native = manifest.native;
  let validatedNative: ExtensionManifest["native"];
  if (native !== undefined) {
    if (!isRecord(native)) {
      throw new Error("manifest.native должен быть объектом");
    }
    const supported = new Set([
      "executable",
      "devExecutable",
      "cargoPackage",
      "args",
      "singleInstance",
    ]);
    const unsupported = Object.keys(native).find((field) => !supported.has(field));
    if (unsupported) throw new Error(`manifest.native.${unsupported} не поддерживается`);
    const executable = optionalPath(native, "executable");
    if (!executable) throw new Error("manifest.native.executable обязателен");
    const args = native.args;
    if (args !== undefined && (!Array.isArray(args) || !args.every(isString))) {
      throw new Error("manifest.native.args должен быть массивом строк");
    }
    const singleInstance = native.singleInstance;
    if (singleInstance !== undefined && !isBoolean(singleInstance)) {
      throw new Error("manifest.native.singleInstance должен быть boolean");
    }
    validatedNative = {
      executable,
      devExecutable: optionalPath(native, "devExecutable"),
      cargoPackage: optionalString(native, "cargoPackage"),
      args,
      singleInstance,
    };
  }

  return {
    ...manifest,
    id: manifest.id,
    name: manifest.name,
    appId,
    version,
    description,
    author,
    kind,
    keplerApiVersion,
    icon,
    entryHtml,
    preload,
    keepAliveInBackground: keepAlive,
    native: validatedNative,
  };
}
