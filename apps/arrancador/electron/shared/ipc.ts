import { ipcRenderer } from "electron";
import type {
  ArrancadorBridge,
  ArrancadorEventCallback,
  ArrancadorEventName,
  IpcArgs,
  IpcChannel,
} from "../../src/types/ipc";

export { ARRANCADOR_BRIDGE_KEY } from "../../src/types/ipc";

type PayloadMode = "none" | "required" | "optional";
type IpcCommandDefinition = { payload: PayloadMode };

export const IPC_COMMAND_REGISTRY = {
  get_all_games: { payload: "none" },
  get_game: { payload: "required" },
  add_game: { payload: "required" },
  add_games_batch: { payload: "required" },
  update_game: { payload: "required" },
  delete_game: { payload: "required" },
  toggle_favorite: { payload: "required" },
  get_favorites: { payload: "none" },
  record_game_launch: { payload: "required" },
  search_games: { payload: "required" },
  game_exists_by_path: { payload: "required" },
  is_game_installed: { payload: "required" },
  launch_game: { payload: "required" },
  get_running_instances: { payload: "required" },
  kill_game_processes: { payload: "required" },
  add_game_process_bindings: { payload: "required" },
  remove_game_process_binding: { payload: "required" },
  resolve_shortcut_target: { payload: "required" },
  list_recent_usage_processes: { payload: "optional" },
  search_usage_processes: { payload: "required" },
  sync_games_to_ark: { payload: "none" },
  migrate_ark_games: { payload: "required" },
  search_rawg: { payload: "required" },
  get_rawg_game_details: { payload: "required" },
  apply_rawg_metadata: { payload: "required" },
  set_rawg_api_key: { payload: "required" },
  get_rawg_api_key: { payload: "none" },
  get_all_achievements: { payload: "required" },
  seed_default_achievements: { payload: "none" },
  record_achievement_event: { payload: "required" },
  check_ludusavi_installed: { payload: "none" },
  get_ludusavi_executable_path: { payload: "none" },
  set_ludusavi_path: { payload: "required" },
  set_backup_directory: { payload: "required" },
  get_backup_directory_setting: { payload: "none" },
  refresh_sqoba_manifest: { payload: "none" },
  find_game_save_paths: { payload: "required" },
  find_game_saves: { payload: "required" },
  create_backup: { payload: "required" },
  get_game_backups: { payload: "required" },
  restore_backup: { payload: "required" },
  delete_backup: { payload: "required" },
  should_backup_before_launch: { payload: "required" },
  check_backup_needed: { payload: "required" },
  check_restore_needed: { payload: "required" },
  get_backup_settings: { payload: "none" },
  update_backup_settings: { payload: "required" },
  get_all_settings: { payload: "none" },
  update_settings: { payload: "required" },
  get_setting: { payload: "required" },
  set_setting: { payload: "required" },
  get_ark_connection_info: { payload: "none" },
  add_scan_directory: { payload: "required" },
  get_scan_directories: { payload: "none" },
  remove_scan_directory: { payload: "required" },
  get_playtime_stats: { payload: "required" },
  get_running_processes: { payload: "none" },
  get_system_info: { payload: "none" },
  test_disk_speed: { payload: "required" },
  list_notifications: { payload: "required" },
  create_notification: { payload: "required" },
  mark_notification_read: { payload: "required" },
  mark_all_notifications_read: { payload: "none" },
  clear_notifications: { payload: "none" },
  get_catalogue_items: { payload: "required" },
  search_catalogue: { payload: "required" },
  upsert_catalogue_item: { payload: "required" },
  delete_catalogue_item: { payload: "required" },
  sync_library_to_catalogue: { payload: "none" },
  dialog_open: { payload: "optional" },
  get_window_platform: { payload: "none" },
  window_minimize: { payload: "none" },
  window_toggle_maximize: { payload: "none" },
  window_close: { payload: "none" },
  shell_open_path: { payload: "required" },
  shell_open_external: { payload: "required" },
  get_autostart_state: { payload: "none" },
  set_autostart_state: { payload: "required" },
  scan_executables_stream: { payload: "required" },
  cancel_scan: { payload: "none" },
} as const satisfies Record<IpcChannel, IpcCommandDefinition>;

export const IPC_EVENT_REGISTRY = {
  "scan:entry": true,
  "scan:done": true,
  "backup:progress": true,
  "restore:progress": true,
  "game:save-path-missing": true,
} as const satisfies Record<ArrancadorEventName, true>;

const IPC_CHANNELS = Object.keys(IPC_COMMAND_REGISTRY) as IpcChannel[];
const ALLOWED_IPC_CHANNELS = new Set<string>(IPC_CHANNELS);
const ALLOWED_EVENT_CHANNELS = new Set<string>(Object.keys(IPC_EVENT_REGISTRY));

type PayloadValidator = (payload: unknown) => void;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireRecord(channel: IpcChannel, payload: unknown) {
  if (!isRecord(payload)) {
    throw new Error(`Invalid IPC payload for ${channel}: expected object`);
  }
  return payload;
}

function requireStringField(
  channel: IpcChannel,
  payload: Record<string, unknown>,
  key: string,
) {
  if (typeof payload[key] !== "string") {
    throw new Error(`Invalid IPC payload for ${channel}: expected string ${key}`);
  }
}

function requireBooleanField(
  channel: IpcChannel,
  payload: Record<string, unknown>,
  key: string,
) {
  if (typeof payload[key] !== "boolean") {
    throw new Error(`Invalid IPC payload for ${channel}: expected boolean ${key}`);
  }
}

function validateStringFields(...keys: string[]): PayloadValidator {
  return (payload) => {
    const channel = currentValidationChannel;
    const record = requireRecord(channel, payload);
    for (const key of keys) {
      requireStringField(channel, record, key);
    }
  };
}

function validateDialogOpen(payload: unknown) {
  const record = requireRecord("dialog_open", payload);
  for (const key of ["directory", "multiple"]) {
    if (record[key] !== undefined && typeof record[key] !== "boolean") {
      throw new Error(`Invalid IPC payload for dialog_open: expected boolean ${key}`);
    }
  }
  if (record.title !== undefined && typeof record.title !== "string") {
    throw new Error("Invalid IPC payload for dialog_open: expected string title");
  }
  if (record.filters !== undefined) {
    if (!Array.isArray(record.filters)) {
      throw new Error("Invalid IPC payload for dialog_open: expected filters array");
    }
    for (const filter of record.filters) {
      if (
        !isRecord(filter) ||
        typeof filter.name !== "string" ||
        !Array.isArray(filter.extensions) ||
        !filter.extensions.every((extension) => typeof extension === "string")
      ) {
        throw new Error("Invalid IPC payload for dialog_open: malformed filter");
      }
    }
  }
}

function validateSetAutostartState(payload: unknown) {
  const record = requireRecord("set_autostart_state", payload);
  requireBooleanField("set_autostart_state", record, "enabled");
}

function validateShellOpenExternal(payload: unknown) {
  const record = requireRecord("shell_open_external", payload);
  requireStringField("shell_open_external", record, "url");
  const value = record.url as string;
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error("Invalid IPC payload for shell_open_external: expected http(s) URL");
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error("Invalid IPC payload for shell_open_external: expected http(s) URL");
  }
}

function validateUsageProcessSearch(payload: unknown) {
  const record = requireRecord("search_usage_processes", payload);
  requireStringField("search_usage_processes", record, "query");
  if (record.limit !== undefined && typeof record.limit !== "number") {
    throw new Error("Invalid IPC payload for search_usage_processes: expected number limit");
  }
}

function validateRecentUsageProcesses(payload: unknown) {
  const record = requireRecord("list_recent_usage_processes", payload);
  if (record.limit !== undefined && typeof record.limit !== "number") {
    throw new Error("Invalid IPC payload for list_recent_usage_processes: expected number limit");
  }
}

function validateGameProcessBindings(payload: unknown) {
  const record = requireRecord("add_game_process_bindings", payload);
  requireStringField("add_game_process_bindings", record, "id");
  if (!Array.isArray(record.bindings)) {
    throw new Error("Invalid IPC payload for add_game_process_bindings: expected bindings array");
  }

  for (const binding of record.bindings) {
    if (
      !isRecord(binding) ||
      (binding.match_type !== "exe_path" && binding.match_type !== "process_name") ||
      typeof binding.match_value !== "string"
    ) {
      throw new Error("Invalid IPC payload for add_game_process_bindings: malformed binding");
    }
  }
}

function validateRemoveGameProcessBinding(payload: unknown) {
  const record = requireRecord("remove_game_process_binding", payload);
  requireStringField("remove_game_process_binding", record, "id");
  if (typeof record.bindingId !== "number") {
    throw new Error("Invalid IPC payload for remove_game_process_binding: expected number bindingId");
  }
}

let currentValidationChannel: IpcChannel = "get_all_games";

const PAYLOAD_VALIDATORS: Partial<Record<IpcChannel, PayloadValidator>> = {
  add_game_process_bindings: validateGameProcessBindings,
  add_scan_directory: validateStringFields("path"),
  dialog_open: validateDialogOpen,
  game_exists_by_path: validateStringFields("exePath"),
  get_setting: validateStringFields("key"),
  list_recent_usage_processes: validateRecentUsageProcesses,
  migrate_ark_games: validateStringFields("sourceDbPath"),
  remove_scan_directory: validateStringFields("path"),
  remove_game_process_binding: validateRemoveGameProcessBinding,
  resolve_shortcut_target: validateStringFields("path"),
  scan_executables_stream: validateStringFields("dir"),
  search_usage_processes: validateUsageProcessSearch,
  set_autostart_state: validateSetAutostartState,
  set_backup_directory: validateStringFields("path"),
  set_ludusavi_path: validateStringFields("path"),
  set_setting: validateStringFields("key", "value"),
  shell_open_external: validateShellOpenExternal,
  shell_open_path: validateStringFields("path"),
  test_disk_speed: validateStringFields("mountPoint"),
};

function assertAllowedIpcChannel(channel: string): asserts channel is IpcChannel {
  if (!ALLOWED_IPC_CHANNELS.has(channel)) {
    throw new Error(`Blocked IPC invoke for unapproved channel: ${channel}`);
  }
}

function assertAllowedEventChannel(
  event: string,
): asserts event is ArrancadorEventName {
  if (!ALLOWED_EVENT_CHANNELS.has(event)) {
    throw new Error(`Blocked IPC listener for unapproved event: ${event}`);
  }
}

function validateIpcPayload<C extends IpcChannel>(
  channel: C,
  payload: IpcArgs<C> | undefined,
) {
  const command = IPC_COMMAND_REGISTRY[channel];

  if (command.payload === "none") {
    if (payload !== undefined) {
      throw new Error(`Invalid IPC payload for ${channel}: expected no payload`);
    }
    return;
  }

  const validator = PAYLOAD_VALIDATORS[channel];
  if (!validator) {
    return;
  }

  currentValidationChannel = channel;
  validator(payload);
}

export const createArrancadorBridge = (
  invoke: (channel: string, payload?: unknown) => Promise<unknown>,
): ArrancadorBridge => {
  const commands = Object.fromEntries(
    IPC_CHANNELS.map((channel) => [
      channel,
      (payload?: unknown) => {
        validateIpcPayload(channel, payload as never);
        return payload === undefined ? invoke(channel) : invoke(channel, payload);
      },
    ]),
  ) as ArrancadorBridge["commands"];

  return {
    commands,
    on: ((event, callback) => {
      assertAllowedEventChannel(event);
      const listener = (_electronEvent: unknown, payload: unknown) => {
        (callback as (value: unknown) => void)(payload);
      };

      ipcRenderer.on(event, listener);
      return () => {
        ipcRenderer.removeListener(event, listener);
      };
    }) as <E extends ArrancadorEventName>(
      event: E,
      callback: ArrancadorEventCallback<E>,
    ) => () => void,
  };
};

export const electronRendererInvoke = (
  channel: string,
  payload?: unknown,
) => {
  assertAllowedIpcChannel(channel);
  validateIpcPayload(channel, payload as never);
  return ipcRenderer.invoke(channel, payload);
};
