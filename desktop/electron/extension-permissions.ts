export type ExtensionSource = "dev" | "user" | "bundled";

type JsonPrimitive = string | number | boolean | null;
export interface JsonRecord {
  [key: string]: JsonValue;
}
export type JsonValue = JsonPrimitive | JsonValue[] | JsonRecord;
export interface ExtensionPermissionCheck {
  extensionId: string;
  source: ExtensionSource;
  manifestPermissions?: readonly string[];
  operation: string;
  params?: JsonRecord;
  resolveObjectType?: (id: string) => Promise<string | null>;
}
export interface ExtensionHostPermissionCheck {
  extensionId: string;
  source: ExtensionSource;
  manifestPermissions?: readonly string[];
  capability:
    | "userData.read"
    | "userData.write"
    | "focus.control"
    | "network.read"
    | "dialogs.directory"
    | "markdownFiles.open"
    | "markdownFiles.save";
}
const TRUSTED_SOURCES = new Set<ExtensionSource>(["dev", "bundled"]);
const OBJECT_READ_OPS = new Set([
  "load_all",
  "list_objects",
  "list_object_summaries",
  "list_objects_by_type",
  "list_object_summaries_by_type",
  "list_running_time_entries",
  "get_objects_by_ids",
  "search_objects",
  "get_object",
  "list_object_types",
  "get_object_type",
  "list_object_links",
]);
const OBJECT_WRITE_OPS = new Set([
  "upsert_object_type",
  "delete_object_type",
  "upsert_object_link",
  "delete_object_link",
  "delete_trashed",
]);
const USAGE_READ_OPS = new Set([
  "get_usage_analytics",
  "list_recent_usage_processes",
  "search_usage_processes",
  "get_usage_game_playtime_summary",
]);
const USAGE_WRITE_OPS = new Set([
  "upsert_tracked_app",
  "delete_tracked_app",
  "upsert_usage_session",
  "delete_usage_session",
  "upsert_usage_event",
  "delete_usage_event",
]);
const isTrustedSource = (source: ExtensionSource): boolean => TRUSTED_SOURCES.has(source);

function hasCapability(granted: readonly string[] | undefined, required: string): boolean {
  if (!granted || granted.length === 0) return false;
  if (granted.includes("*") || granted.includes(required)) return true;
  if (
    (required === "userData.read" && granted.includes("filesystem.read")) ||
    (required === "userData.write" && granted.includes("filesystem.write"))
  )
    return true;
  if (required.startsWith("objects.write:") && granted.includes("objects.write")) return true;
  return false;
}

function hasAnyCapability(
  granted: readonly string[] | undefined,
  required: readonly string[],
): boolean {
  return required.some((capability) => hasCapability(granted, capability));
}
export function isRecord(value: JsonValue | undefined): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}

function objectTypeFromParams(params: JsonRecord | undefined): string | null {
  const object = params?.object;
  if (!isRecord(object)) return null;
  const record = object;
  const raw = record.type_id ?? record.typeId;
  return isString(raw) && raw.length > 0 ? raw : null;
}
function commandIdsFromParams(operation: string, params: JsonRecord | undefined): string[] {
  if (operation === "commands.invoke") {
    return isString(params?.id) ? [params.id] : [];
  }
  if (operation === "commands.unregister") {
    return Array.isArray(params?.ids) ? params.ids.filter(isString) : [];
  }
  if (operation !== "commands.register") return [];
  const commands = params?.commands;
  if (!Array.isArray(commands)) return [];
  return commands
    .map((command) => {
      if (!isRecord(command)) return null;
      const id = command.id;
      return isString(id) ? id : null;
    })
    .filter((id): id is string => Boolean(id));
}

function assertOwnCommandNamespace(
  extensionId: string,
  operation: string,
  ids: readonly string[],
): void {
  if (!operation.startsWith("commands.") || ids.length === 0) return;
  const prefix = `${extensionId}:`;
  const foreign = ids.find((id) => !id.startsWith(prefix));
  if (foreign) {
    throw new Error(
      `[kepler-shell] extension '${extensionId}' cannot use command id outside its namespace: ${foreign}`,
    );
  }
}

function requiredCapabilities(operation: string, params?: JsonRecord): string[] {
  if (OBJECT_READ_OPS.has(operation)) return ["objects.read"];
  if (OBJECT_WRITE_OPS.has(operation)) return ["objects.write"];
  if (operation === "upsert_object") {
    const typeId = objectTypeFromParams(params);
    return typeId ? [`objects.write:${typeId}`, "objects.write"] : ["objects.write"];
  }
  if (operation === "delete_object") return ["objects.write"];
  if (USAGE_READ_OPS.has(operation)) return ["usage.read"];
  if (USAGE_WRITE_OPS.has(operation)) return ["usage.write"];
  if (operation === "commands.register" || operation === "commands.unregister") {
    return ["commands.register"];
  }
  if (operation === "commands.invoke" || operation === "commands.list") return ["commands.invoke"];
  if (operation.startsWith("focus.")) return ["focus.control"];
  if (operation.startsWith("pomodoro.")) return ["pomodoro.control", "focus.control"];
  if (operation === "arrancador.add_manual") return ["objects.write:game_obj"];
  if (
    operation === "arrancador.list" ||
    operation === "arrancador.read" ||
    operation === "arrancador.scan" ||
    operation === "arrancador.rawg.search"
  )
    return ["arrancador.scan"];
  if (operation.startsWith("arrancador.rawg.")) return ["objects.write:game_obj"];
  if (operation.startsWith("arrancador.sqoba.")) return ["arrancador.launch"];
  if (operation.startsWith("arrancador.config.")) return ["arrancador.scan"];
  if (operation === "arrancador.launch") {
    return ["arrancador.launch"];
  }
  if (operation.startsWith("dictation.")) return ["dictation.control"];
  if (
    operation === "agents.projects.list" ||
    operation === "agents.sessions.list" ||
    operation === "agents.sessions.get" ||
    operation === "agents.sessions.timeline" ||
    operation === "agents.diff.get" ||
    operation === "agents.models.list" ||
    operation === "agents.editors.list" ||
    operation === "agents.snapshot"
  ) {
    return ["agents.read"];
  }
  if (operation.startsWith("agents.")) return ["agents.control"];
  if (
    operation === "app_index.list_all" ||
    operation === "app_index.search" ||
    operation === "file_index.search" ||
    operation === "file_index.settings_get"
  ) {
    return ["hostIndex.read"];
  }
  if (
    operation === "app_index.launch" ||
    operation === "app_index.rescan" ||
    operation === "file_index.open" ||
    operation === "file_index.rescan" ||
    operation.startsWith("file_index.scope_") ||
    operation.startsWith("file_index.ignore_") ||
    operation === "file_index.settings_set"
  ) {
    return ["hostIndex.write"];
  }
  if (operation === "export.list") return ["export.read"];
  if (operation === "export.run") return ["export.run"];
  if (
    operation === "set_sync_kv" ||
    operation === "clear_all" ||
    operation === "start_sync" ||
    operation === "stop_sync" ||
    operation === "broadcast_change" ||
    operation === "add_seed_peer" ||
    operation === "leave_space"
  ) {
    return ["sync.admin"];
  }
  if (
    operation === "get_sync_kv" ||
    operation === "get_connected_peers" ||
    operation === "get_own_addresses" ||
    operation === "get_host_device_name"
  ) {
    return ["sync.read"];
  }
  return [];
}

async function deleteObjectCapabilities(check: ExtensionPermissionCheck): Promise<string[]> {
  if (hasCapability(check.manifestPermissions, "objects.write")) return ["objects.write"];
  const id = check.params?.id;
  if (!isString(id) || !check.resolveObjectType) return ["objects.write"];
  const typeId = await check.resolveObjectType(id);
  return typeId ? [`objects.write:${typeId}`, "objects.write"] : ["objects.write"];
}

export async function assertExtensionArkPermission(check: ExtensionPermissionCheck): Promise<void> {
  if (isTrustedSource(check.source)) return;
  if (!check.operation || check.operation.length === 0) {
    throw new Error("[kepler-shell] extension ARK operation must be a non-empty string");
  }
  if (check.operation === "delete_object") {
    const required = await deleteObjectCapabilities(check);
    if (hasAnyCapability(check.manifestPermissions, required)) return;
    throwPermissionError(check.extensionId, check.operation, required);
  }
  const commandIds = commandIdsFromParams(check.operation, check.params);
  assertOwnCommandNamespace(check.extensionId, check.operation, commandIds);
  const required = requiredCapabilities(check.operation, check.params);
  if (required.length === 0 || !hasAnyCapability(check.manifestPermissions, required)) {
    throwPermissionError(check.extensionId, check.operation, required);
  }
}

export function assertExtensionHostPermission(check: ExtensionHostPermissionCheck): void {
  if (isTrustedSource(check.source)) return;
  const declared =
    check.capability === "userData.read"
      ? ["userData.read", "filesystem.read"]
      : check.capability === "userData.write"
        ? ["userData.write", "filesystem.write"]
        : [check.capability];
  if (!hasAnyCapability(check.manifestPermissions, declared)) {
    throwPermissionError(check.extensionId, check.capability, declared);
  }
}

export function assertExtensionEventPermission(check: {
  extensionId: string;
  source: ExtensionSource;
  manifestPermissions?: readonly string[];
  event: string;
}): void {
  if (isTrustedSource(check.source)) return;
  const required = requiredEventCapabilities(check.event);
  if (required.length === 0 || !hasAnyCapability(check.manifestPermissions, required)) {
    throwPermissionError(check.extensionId, `event:${check.event}`, required);
  }
}
function requiredEventCapabilities(event: string): string[] {
  if (["dictation.trigger", "dictation.models_changed", "dictation.error"].includes(event))
    return ["dictation.control"];
  if (
    event === "entity_changed" ||
    event === "object_upserted" ||
    event === "object_deleted" ||
    event === "sync_replay"
  ) {
    return ["objects.read"];
  }
  if (event === "command_invoked" || event === "commands_changed") {
    return ["commands.invoke", "commands.register"];
  }
  if (event.startsWith("pomodoro_")) return ["focus.control"];
  if (event === "agents_event") return ["agents.read"];
  if (event === "peer_list_updated" || event === "sync_error") return ["sync.read"];
  return [];
}
function throwPermissionError(
  extensionId: string,
  operation: string,
  required: readonly string[],
): never {
  const suffix = required.length > 0 ? `; required: ${required.join(" or ")}` : "";
  throw new Error(
    `[kepler-shell] extension '${extensionId}' is not allowed to call ${operation}${suffix}`,
  );
}
