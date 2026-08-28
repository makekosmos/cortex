import { isRecord, isString } from "../src/shared/runtimeGuards";

export interface ExtensionManifest {
  id: string;
  appId?: string;
  name: string;
  kind?: "app" | "native" | string;
  version?: string;
  description?: string;
  author?: string;
  icon?: string;
  entryHtml?: string;
  keplerApiVersion?: string;
  keepAliveInBackground?: boolean;
  native?: {
    executable?: string;
    devExecutable?: string;
  };
}

/**
 * Validate the package manifest before deriving paths or loading optional fields.
 * This is deliberately shared by archive and directory installs so malformed
 * first-party artifacts fail closed with the same contract.
 */
export function validateExtensionManifest(value: unknown): ExtensionManifest {
  if (!isRecord(value)) {
    throw new Error("manifest.json должен содержать объект");
  }
  if (!isString(value.id) || !value.id) {
    throw new Error("manifest.id обязателен и должен быть строкой");
  }
  if (!/^\w[\w.-]*$/.test(value.id)) {
    throw new Error(`manifest.id невалиден: ${value.id}`);
  }
  if (!isString(value.name) || !value.name.trim()) {
    throw new Error("manifest.name обязателен и должен быть строкой");
  }
  // SAFETY: required id/name checks establish the ExtensionManifest boundary.
  return value as ExtensionManifest;
}
