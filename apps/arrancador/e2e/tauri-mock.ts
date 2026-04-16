type MockCatalogueItem = {
  id: number;
  rawg_id: number;
  name: string;
  source: string;
  payload: string;
  updated_at: string;
};

type BridgeMockConfig = {
  games: Array<Record<string, unknown>>;
  settings: Record<string, unknown>;
  scanEntries: Array<{ path: string; file_name: string }>;
  processes: Array<{
    pid: number;
    name: string;
    path: string;
    cpu_usage: number;
    gpu_usage: number;
  }>;
  dialogOpenResult?: string | string[] | null;
  catalogueItems?: MockCatalogueItem[];
};

const stableHash = (value: string) => {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash << 5) - hash + value.charCodeAt(index);
    hash |= 0;
  }
  return Math.abs(hash);
};

const now = () => new Date().toISOString();

export const bridgeMockInit = (config: BridgeMockConfig) => {
  const listeners = new Map<string, Map<string, string>>();
  const callbacks = new Map<
    string,
    { callback: (payload: unknown) => void; once: boolean }
  >();
  let callbackId = 0;
  let eventId = 0;

  let catalogueNextId =
    (config.catalogueItems ?? []).reduce(
      (max, item) => Math.max(max, item.id),
      0,
    ) + 1;

  const state = {
    games: config.games ? [...config.games] : [],
    settings: config.settings ? { ...config.settings } : {},
    catalogue: config.catalogueItems
      ? [...config.catalogueItems]
      : ([] as MockCatalogueItem[]),
  };

  const registerListener = (event: string, handlerId: string) => {
    const id = `listener-${++eventId}`;
    if (!listeners.has(event)) {
      listeners.set(event, new Map());
    }
    listeners.get(event)?.set(id, handlerId);
    return id;
  };

  const emit = (event: string, payload: unknown) => {
    const eventListeners = listeners.get(event);
    if (!eventListeners) return;

    for (const [listenerId, handlerId] of eventListeners.entries()) {
      const handler = callbacks.get(String(handlerId));
      if (!handler) continue;

      handler.callback({ event, id: listenerId, payload });
      if (handler.once) {
        callbacks.delete(String(handlerId));
        eventListeners.delete(listenerId);
      }
    }
  };

  const getCatalogueItems = (source?: string | null) => {
    const list = source
      ? state.catalogue.filter((item) => item.source === source)
      : state.catalogue;

    return [...list].sort((a, b) => a.name.localeCompare(b.name));
  };

  const searchCatalogue = (query: string, source?: string | null) => {
    const trimmed = (query ?? "").trim().toLowerCase();
    if (!trimmed) {
      return getCatalogueItems(source);
    }

    const list = getCatalogueItems(source);
    return list.filter((item) => {
      const haystack = `${item.name} ${item.payload} ${item.source}`.toLowerCase();
      return haystack.includes(trimmed);
    });
  };

  const upsertCatalogue = (args: {
    id?: number;
    rawg_id: number;
    name: string;
    source?: string;
    payload: string;
  }) => {
    const source = (args.source ?? "rawg").trim() || "rawg";
    const nowValue = now();
    const item: MockCatalogueItem = {
      id: args.id ?? catalogueNextId++,
      rawg_id: args.rawg_id,
      name: args.name,
      source,
      payload: args.payload,
      updated_at: nowValue,
    };

    if (args.id != null) {
      const index = state.catalogue.findIndex((entry) => entry.id === args.id);
      if (index === -1) {
        throw new Error("Item not found");
      }
      state.catalogue[index] = item;
      return item;
    }

    const existing = state.catalogue.find(
      (entry) => entry.source === source && entry.rawg_id === args.rawg_id,
    );
    if (existing) {
      existing.name = item.name;
      existing.payload = item.payload;
      existing.updated_at = nowValue;
      return existing;
    }

    state.catalogue = [item, ...state.catalogue];
    return item;
  };

  const syncCatalogueFromLibrary = () => {
    const stamp = now();
    let synced = 0;
    for (const rawGame of state.games) {
      const game = rawGame as { id?: string; name?: string };
      const rawgId = stableHash(String(game.id ?? ""));

      const existing = state.catalogue.find(
        (item) => item.source === "library" && item.rawg_id === rawgId,
      );
      const payload = JSON.stringify({
        source: "library",
        source_game_id: game.id ?? null,
        synced_at: stamp,
      });

      if (existing) {
        existing.name = game.name ?? existing.name;
        existing.payload = payload;
        existing.updated_at = stamp;
      } else {
        state.catalogue.unshift({
          id: catalogueNextId++,
          rawg_id: rawgId,
          name: String(game.name ?? ""),
          source: "library",
          payload,
          updated_at: stamp,
        });
        synced += 1;
      }
    }

    return { synced };
  };

  const invokeHandlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    get_all_games: () => state.games,
    get_all_settings: () => state.settings,
    update_settings: (args) => {
      state.settings = {
        ...state.settings,
        ...(args.settings as Record<string, unknown>),
      };
      return null;
    },
    set_rawg_api_key: () => null,
    game_exists_by_path: (args) =>
      state.games.some((game) => game.exe_path === args.exePath),
    get_running_processes: () => config.processes ?? [],
    scan_executables_stream: () => {
      setTimeout(() => {
        (config.scanEntries ?? []).forEach((entry) => {
          emit("scan:entry", entry);
        });
        emit("scan:done", null);
      }, 50);
      return null;
    },
    cancel_scan: () => {
      emit("scan:done", null);
      return null;
    },
    "plugin:autostart|is_enabled": () => false,
    "plugin:autostart|enable": () => null,
    "plugin:autostart|disable": () => null,
    "plugin:dialog|open": () => config.dialogOpenResult ?? null,
    get_catalogue_items: (args) => {
      const source = args?.source;
      return getCatalogueItems(typeof source === "string" ? source : null);
    },
    search_catalogue: (args) => {
      const query = String(args?.query ?? "");
      const source = args?.source;
      return searchCatalogue(query, typeof source === "string" ? source : null);
    },
    upsert_catalogue_item: (args) => {
      const payload = String(args?.payload ?? "{}");
      const parsed = JSON.parse(payload);
      if (typeof args.rawg_id !== "number" || typeof args.name !== "string") {
        throw new Error("Invalid catalogue payload");
      }
      return upsertCatalogue({
        id: typeof args.id === "number" ? args.id : undefined,
        rawg_id: args.rawg_id,
        name: args.name,
        source: typeof args.source === "string" ? (args.source as string) : undefined,
        payload: JSON.stringify(parsed),
      });
    },
    delete_catalogue_item: (args) => {
      const id = Number(args.id);
      const start = state.catalogue.length;
      state.catalogue = state.catalogue.filter((item) => item.id !== id);
      return state.catalogue.length !== start;
    },
    sync_library_to_catalogue: () => syncCatalogueFromLibrary(),
  };

  const invoke = async (cmd: string, args: Record<string, unknown> = {}) => {
    if (cmd === "plugin:event|listen") {
      const event = String(args.event ?? "");
      const handlerId = String(args.handler ?? "");
      return registerListener(event, handlerId);
    }

    if (cmd === "plugin:event|unlisten") {
      const event = String(args.event ?? "");
      const listenerId = String(args.eventId ?? "");
      listeners.get(event)?.delete(listenerId);
      return null;
    }

    const handler = invokeHandlers[cmd];
    return handler ? handler(args) : null;
  };

  const tauriWindow = globalThis as typeof globalThis & {
    arrancador?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
    __TAURI_INTERNALS__: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      transformCallback: (
        callback: (payload: unknown) => void,
        once?: boolean,
      ) => string;
      unregisterCallback: (id: string) => void;
    };
    __TAURI_EVENT_PLUGIN_INTERNALS__: {
      unregisterListener: (event: string, eventId: string) => void;
    };
    __TAURI_MOCK__: { emit: typeof emit; state: typeof state };
  };

  tauriWindow.arrancador = {
    invoke,
    on: () => () => {},
  };
  tauriWindow.__TAURI_MOCK__ = { emit, state };
  tauriWindow.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (callback, once = false) => {
      const id = `callback-${++callbackId}`;
      callbacks.set(id, { callback, once });
      return id;
    },
    unregisterCallback: (id) => {
      callbacks.delete(id);
    },
  };
  tauriWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event, listenerId) => {
      listeners.get(event)?.delete(listenerId);
    },
  };
};
