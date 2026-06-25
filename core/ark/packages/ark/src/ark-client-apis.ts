import type {
  ArkCommandsApi,
  ArkKvApi,
  ArkLinksApi,
  ArkObjectLinkRecord,
  ArkObjectRecord,
  ArkObjectSummaryRecord,
  ArkObjectsApi,
  ArkObjectTypeRecord,
  ArkObjectTypesApi,
  ArkSearchResult,
  CommandInvokedCallback,
  CommandManifest,
  CommandsChangedCallback,
  SidecarRequest,
} from "./ark-client.types.js";
import type {
  ArkLoadAllData,
  ArkUsageAnalyticsSnapshot,
  ArkUsageApi,
  ArkUsageGamePlaytimeSummary,
  ArkUsageProcessCandidate,
} from "./ark-client-usage.types.js";

type RequestAfterInit = <T>(request: SidecarRequest) => Promise<T>;

export interface ArkClientApis {
  objects: ArkObjectsApi;
  objectTypes: ArkObjectTypesApi;
  links: ArkLinksApi;
  usage: ArkUsageApi;
  kv: ArkKvApi;
  commands: ArkCommandsApi;
}

export interface ArkClientApisOptions {
  deviceId: string;
  requestAfterInit: RequestAfterInit;
  commandInvokedCallbacks: Set<CommandInvokedCallback>;
  commandsChangedCallbacks: Set<CommandsChangedCallback>;
}

export function createArkClientApis({
  deviceId,
  requestAfterInit,
  commandInvokedCallbacks,
  commandsChangedCallbacks,
}: ArkClientApisOptions): ArkClientApis {
  const objects: ArkObjectsApi = {
    list: () => requestAfterInit<ArkObjectRecord[]>({ operation: "list_objects" }),
    listSummaries: () =>
      requestAfterInit<ArkObjectSummaryRecord[]>({
        operation: "list_object_summaries",
      }),
    listByType: (typeId) =>
      requestAfterInit<ArkObjectRecord[]>({
        operation: "list_objects_by_type",
        type_id: typeId,
      }),
    listSummariesByType: (typeId) =>
      requestAfterInit<ArkObjectSummaryRecord[]>({
        operation: "list_object_summaries_by_type",
        type_id: typeId,
      }),
    listRunningTimeEntries: (opts) =>
      requestAfterInit<ArkObjectRecord[]>({
        operation: "list_running_time_entries",
        ...(opts?.source ? { source: opts.source } : {}),
      }),
    get: (id) => requestAfterInit<ArkObjectRecord | null>({ operation: "get_object", id }),
    getMany: (ids) =>
      requestAfterInit<ArkObjectRecord[]>({
        operation: "get_objects_by_ids",
        ids: [...ids],
      }),
    upsert: async (object) => {
      await requestAfterInit<boolean>({
        operation: "upsert_object",
        object,
        device_id: deviceId,
      });
    },
    delete: async (id) => {
      await requestAfterInit<boolean>({
        operation: "delete_object",
        id,
        device_id: deviceId,
      });
    },
    search: (query) =>
      requestAfterInit<ArkSearchResult[]>({
        operation: "search_objects",
        query,
      }),
  };
  const objectTypes: ArkObjectTypesApi = {
    list: () =>
      requestAfterInit<ArkObjectTypeRecord[]>({
        operation: "list_object_types",
      }),
    get: (id) =>
      requestAfterInit<ArkObjectTypeRecord | null>({
        operation: "get_object_type",
        id,
      }),
    upsert: async (objectType) => {
      await requestAfterInit<boolean>({
        operation: "upsert_object_type",
        object_type: objectType,
        device_id: deviceId,
      });
    },
    delete: async (id) => {
      await requestAfterInit<boolean>({
        operation: "delete_object_type",
        id,
        device_id: deviceId,
      });
    },
  };
  const links: ArkLinksApi = {
    list: () =>
      requestAfterInit<ArkObjectLinkRecord[]>({
        operation: "list_object_links",
      }),
    upsert: async (link) => {
      await requestAfterInit<boolean>({
        operation: "upsert_object_link",
        object_link: link,
        device_id: deviceId,
      });
    },
    delete: async (id) => {
      await requestAfterInit<boolean>({
        operation: "delete_object_link",
        id,
        device_id: deviceId,
      });
    },
  };
  const kv: ArkKvApi = {
    get: (key) => requestAfterInit<string | null>({ operation: "get_sync_kv", key }),
    set: async (key, value) => {
      await requestAfterInit<boolean>({ operation: "set_sync_kv", key, value });
    },
  };
  const usage: ArkUsageApi = {
    loadAll: async () => {
      const data = await requestAfterInit<ArkLoadAllData>({
        operation: "load_all",
      });
      return {
        trackedApps: data.trackedApps ?? [],
        usageSessions: data.usageSessions ?? [],
        usageEvents: data.usageEvents ?? [],
      };
    },
    analytics: {
      snapshot: (options = {}) =>
        requestAfterInit<ArkUsageAnalyticsSnapshot>({
          operation: "get_usage_analytics",
          range_days: options.rangeDays,
          top_apps_limit: options.topAppsLimit,
          recent_sessions_limit: options.recentSessionsLimit,
        }),
    },
    processes: {
      recent: (limit = 10) =>
        requestAfterInit<ArkUsageProcessCandidate[]>({
          operation: "list_recent_usage_processes",
          limit,
        }),
      search: (query, limit = 10) =>
        requestAfterInit<ArkUsageProcessCandidate[]>({
          operation: "search_usage_processes",
          query,
          limit,
        }),
    },
    gamePlaytime: {
      summary: (options) =>
        requestAfterInit<ArkUsageGamePlaytimeSummary>({
          operation: "get_usage_game_playtime_summary",
          bindings: options.bindings,
          range_start: options.rangeStart,
          range_end: options.rangeEnd,
        }),
    },
    trackedApps: {
      upsert: async (record) => {
        await requestAfterInit<boolean>({
          operation: "upsert_tracked_app",
          tracked_app: record,
          device_id: deviceId,
        });
      },
      delete: async (id) => {
        await requestAfterInit<boolean>({
          operation: "delete_tracked_app",
          id,
          device_id: deviceId,
        });
      },
    },
    sessions: {
      upsert: async (record) => {
        await requestAfterInit<boolean>({
          operation: "upsert_usage_session",
          usage_session: record,
          device_id: deviceId,
        });
      },
      delete: async (id) => {
        await requestAfterInit<boolean>({
          operation: "delete_usage_session",
          id,
          device_id: deviceId,
        });
      },
    },
    events: {
      upsert: async (record) => {
        await requestAfterInit<boolean>({
          operation: "upsert_usage_event",
          usage_event: record,
          device_id: deviceId,
        });
      },
      delete: async (id) => {
        await requestAfterInit<boolean>({
          operation: "delete_usage_event",
          id,
          device_id: deviceId,
        });
      },
    },
  };
  const commands: ArkCommandsApi = {
    register: async (commands) => {
      await requestAfterInit<boolean>({
        operation: "commands.register",
        commands,
      });
    },
    unregister: async (ids) => {
      await requestAfterInit<boolean>({
        operation: "commands.unregister",
        ids,
      });
    },
    list: async () => {
      const result = await requestAfterInit<unknown>({
        operation: "commands.list",
      });
      // Backend envelope: { ok, data: { commands: [...] } }. SDK resolves
      // pending.resolve(resp.data) → result === { commands: [...] }.
      // Defensive: некоторые fallback-пути ранее возвращали bare array.
      if (Array.isArray(result)) {
        return result as CommandManifest[];
      }
      if (result && typeof result === "object" && "commands" in result) {
        const arr = (result as { commands: unknown }).commands;
        if (Array.isArray(arr)) return arr as CommandManifest[];
      }
      return [];
    },
    invoke: async (id, params) => {
      await requestAfterInit<boolean>({
        operation: "commands.invoke",
        id,
        ...(params !== undefined ? { params } : {}),
      });
    },
    onInvoked: (handler) => {
      commandInvokedCallbacks.add(handler);
      return () => {
        commandInvokedCallbacks.delete(handler);
      };
    },
    onChanged: (handler) => {
      commandsChangedCallbacks.add(handler);
      return () => {
        commandsChangedCallbacks.delete(handler);
      };
    },
  };

  return { objects, objectTypes, links, usage, kv, commands };
}
