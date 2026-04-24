type MockCatalogueItem = {
  id: number;
  rawg_id: number;
  name: string;
  source: string;
  payload: string;
  updated_at: string;
};

type MockRawgGame = {
  id: number;
  name: string;
  slug: string;
  released: string | null;
  background_image: string | null;
  metacritic: number | null;
  rating: number | null;
  ratings_count: number | null;
  genres: { id: number; name: string; slug: string }[] | null;
  platforms: Array<{
    platform: { id: number; name: string; slug: string };
    requirements_en?: string | null;
    requirements_ru?: string | null;
  }> | null;
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
  rawgItems?: MockRawgGame[];
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
    games: config.games
      ? config.games.map((game) => ({
          ...game,
          process_bindings: Array.isArray(game.process_bindings)
            ? game.process_bindings
            : [],
        }))
      : [],
    settings: config.settings ? { ...config.settings } : {},
    catalogue: config.catalogueItems
      ? [...config.catalogueItems]
      : ([] as MockCatalogueItem[]),
    rawgItems: config.rawgItems ? [...config.rawgItems] : ([] as MockRawgGame[]),
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

      handler.callback(payload);
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

  const searchRawg = (query: string) => {
    const needle = query.trim().toLowerCase();
    const items = state.rawgItems;
    if (!needle) return items;

    return items.filter((item) =>
      `${item.name} ${item.slug}`.toLowerCase().includes(needle),
    );
  };

  const invokeHandlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    get_all_games: () => state.games,
    add_game: (args) => {
      const game = (args.game ?? {}) as Record<string, unknown>;
      const id = `game-${state.games.length + 1}`;
      const created = {
        id,
        name: String(game.name ?? ""),
        exe_path: String(game.exe_path ?? ""),
        exe_name: String(game.exe_name ?? ""),
        process_bindings: [],
        rawg_id: null,
        description: null,
        released: null,
        background_image: null,
        metacritic: null,
        rating: null,
        genres: null,
        platforms: null,
        developers: null,
        publishers: null,
        cover_image: null,
        icon_image: null,
        is_favorite: false,
        play_count: 0,
        total_playtime: 0,
        play_status: "not_started",
        last_played: null,
        date_added: now(),
        backup_enabled: true,
        last_backup: null,
        backup_count: 0,
        save_path: null,
        user_rating: null,
        user_note: null,
      };
      state.games = [...state.games, created];
      return created;
    },
    delete_game: (args) => {
      state.games = state.games.filter((game) => game.id !== args.id);
      return null;
    },
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
    search_rawg: (args) => searchRawg(String(args.query ?? "")),
    apply_rawg_metadata: (args) => {
      const game = state.games.find((item) => item.id === args.gameId);
      const rawg = state.rawgItems.find((item) => item.id === args.rawgId);
      if (!game || !rawg) {
        throw new Error("Game or RAWG item not found");
      }

      Object.assign(game, {
        rawg_id: rawg.id,
        name: args.rename ? rawg.name : game.name,
        released: rawg.released,
        background_image: rawg.background_image,
        metacritic: rawg.metacritic,
        rating: rawg.rating,
        genres: rawg.genres?.map((genre) => genre.name).join(", ") ?? null,
        platforms:
          rawg.platforms
            ?.map((platform) => platform.platform.name)
            .join(", ") ?? null,
      });
      return game;
    },
    get_autostart_state: () => false,
    set_autostart_state: () => null,
    dialog_open: () => config.dialogOpenResult ?? null,
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
      const input = (args?.input ?? {}) as Record<string, unknown>;
      const payload = String(input.payload ?? "{}");
      const parsed = JSON.parse(payload);
      if (typeof input.rawg_id !== "number" || typeof input.name !== "string") {
        throw new Error("Invalid catalogue payload");
      }
      return upsertCatalogue({
        id: typeof input.id === "number" ? input.id : undefined,
        rawg_id: input.rawg_id,
        name: input.name,
        source:
          typeof input.source === "string" ? (input.source as string) : undefined,
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

  const mockWindow = globalThis as typeof globalThis & {
    arrancador?: {
      commands: Record<
        string,
        (args?: Record<string, unknown>) => Promise<unknown>
      >;
      on: (event: string, callback: (payload: unknown) => void) => () => void;
    };
    __ARRANCADOR_BRIDGE_MOCK__: { emit: typeof emit; state: typeof state };
  };

  mockWindow.arrancador = {
    commands: new Proxy(
      {},
      {
        get: (_target, property) => {
          if (typeof property !== "string") {
            return undefined;
          }
          return (args?: Record<string, unknown>) => invoke(property, args);
        },
      },
    ) as Record<string, (args?: Record<string, unknown>) => Promise<unknown>>,
    on: (event, callback) => {
      const id = `callback-${++callbackId}`;
      callbacks.set(id, { callback, once: false });
      const listenerId = registerListener(event, id);
      return () => {
        callbacks.delete(id);
        listeners.get(event)?.delete(listenerId);
      };
    },
  };
  mockWindow.__ARRANCADOR_BRIDGE_MOCK__ = { emit, state };
};
