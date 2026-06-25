import type { KeplerLockInfo } from "./ensure-kepler.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ArkClientOptions {
  spaceId: string;
  deviceId: string;
  deviceName?: string;
  port?: number;
  relayUrl?: string;
  relayApiKey?: string;
  /**
   * Step 4a (iroh-spike): select the iroh p2p QUIC transport instead of
   * relay/LAN. Only effective when the ark-core-rpc sidecar was built with
   * the `iroh-spike` Rust feature — a build without it rejects `start_sync`
   * with an explicit error when `useIroh` is set, rather than silently
   * falling back to relay/LAN. Existing `relayUrl`/`relayApiKey` callers are
   * unaffected: `useIroh` defaults to falsy and wins over `relayUrl` only
   * when explicitly set.
   */
  useIroh?: boolean;
  /**
   * Pairing ticket string for the iroh peer to connect to (see
   * `IrohTransport::our_ticket()`/`from_ticket()` on the Rust side). Only
   * read when `useIroh` is set.
   */
  irohPeerTicket?: string;
  /**
   * Optional shared secret for ARK LAN/P2P hello HMAC authentication.
   *
   * When provided, ark-core-rpc includes an HMAC proof in outbound hello
   * messages and requires the same proof from inbound peers configured with a
   * secret. This authenticates peers; it does not encrypt WebSocket traffic.
   */
  authSecret?: string;
  /**
   * SQLite database path passed to ark-core-rpc during `init`.
   *
   * Required when `requestFn` is not provided. Ignored when an external
   * sidecar request function is injected because that owner is responsible
   * for initializing and switching databases.
   */
  dbPath?: string;
  /**
   * Timeout for requests sent to a self-managed ark-core-rpc child process.
   *
   * Injected `requestFn` mode is not wrapped; the injected owner controls its
   * own timeout policy.
   */
  requestTimeoutMs?: number;
  /**
   * Absolute path to the ark-core-rpc binary.
   *
   * When `requestFn` is provided, `sidecarPath` is ignored (the binary is
   * assumed to be already managed externally, e.g. by `sidecar.ts`).
   */
  sidecarPath?: string;
  /**
   * Optional custom request function that replaces the built-in sidecar
   * child-process management.  Inject this when the calling process already
   * manages the ark-core-rpc sidecar (e.g. Electron's `sidecar.ts`) to avoid
   * spawning a second process for DB + sync.
   *
   * Signature matches `SidecarClient.request<T>(req) => Promise<T>`.
   */
  requestFn?: <T>(req: Record<string, unknown>) => Promise<T>;
  /**
   * Optional function to subscribe to async sidecar events.
   * Must call the provided callback for each event object.
   * Returns an unsubscribe function.
   */
  onEventFn?: (listener: (event: SidecarEvent) => void) => () => void;
  /**
   * Phase 2: Kepler mode. Когда задан — ArkClient не спавнит собственный
   * ark-core-rpc child, а коннектится к долгоиграющему Kepler host через
   * WebSocket. Получается из `ensureKeplerRunning()` helper.
   *
   * Wire-протокол JSON-RPC тот же что у self-managed, плюс hello-handshake
   * (token + pid + protocolVersion) на первом сообщении.
   */
  keplerLock?: KeplerLockInfo;
  /** Override PID для PID-binding handshake. Default — `process.pid`. */
  keplerPidForHandshake?: number;
}

export interface SidecarRequest {
  operation: string;
  [key: string]: unknown;
}

export interface SidecarResponse<T> {
  id?: string;
  ok: boolean;
  data?: T;
  error?: string;
}

export interface SidecarEvent {
  event: string;
  [key: string]: unknown;
}

export interface PendingRequest<T> {
  id: string;
  request: SidecarRequest;
  resolve: (value: T) => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
}

export interface ConnectedPeer {
  device_id: string;
  device_name: string;
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: JsonValue;
  propsJson: JsonValue;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

export interface ArkObjectSummaryRecord {
  id: string;
  typeId: string;
  title: string;
  propsJson: JsonValue;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

export interface CommandManifest {
  id: string;
  title: string;
  subtitle?: string;
  category: "open" | "action";
}

export interface CommandInvokedEvent {
  id: string;
  params?: Record<string, unknown>;
  invokerClientId?: string;
}

export type CommandInvokedCallback = (event: CommandInvokedEvent) => void;
export type CommandsChangedCallback = (commands: CommandManifest[]) => void;

export interface ArkCommandsApi {
  register(commands: CommandManifest[]): Promise<void>;
  unregister(ids: string[]): Promise<void>;
  list(): Promise<CommandManifest[]>;
  invoke(id: string, params?: Record<string, unknown>): Promise<void>;
  /** Подписаться на событие command_invoked. Возвращает unsubscribe-функцию. */
  onInvoked(handler: CommandInvokedCallback): () => void;
  /** Подписаться на event commands_changed (изменился список доступных). */
  onChanged(handler: CommandsChangedCallback): () => void;
}

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

export interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

export interface ArkSearchResult {
  file: string;
  line: number;
  text: string;
  entryId: string;
}

export interface ArkObjectsApi {
  list(): Promise<ArkObjectRecord[]>;
  listSummaries(): Promise<ArkObjectSummaryRecord[]>;
  listByType(typeId: string): Promise<ArkObjectRecord[]>;
  listSummariesByType(typeId: string): Promise<ArkObjectSummaryRecord[]>;
  /**
   * Возвращает только running time_entry_obj (props.endedAt IS NULL),
   * опционально отфильтрованные по props.source. SQL-уровневый фильтр
   * через json_extract — без обхода всех записей типа.
   *
   * Hot path для focus session and widget rehydrate.
   * widget'а (stopManualStopwatch).
   */
  listRunningTimeEntries(opts?: {
    source?: "manual" | "pomodoro" | "pomodoro_break";
  }): Promise<ArkObjectRecord[]>;
  get(id: string): Promise<ArkObjectRecord | null>;
  getMany(ids: readonly string[]): Promise<ArkObjectRecord[]>;
  upsert(object: ArkObjectRecord): Promise<void>;
  delete(id: string): Promise<void>;
  search(query: string): Promise<ArkSearchResult[]>;
}

export interface ArkObjectTypesApi {
  list(): Promise<ArkObjectTypeRecord[]>;
  get(id: string): Promise<ArkObjectTypeRecord | null>;
  upsert(objectType: ArkObjectTypeRecord): Promise<void>;
  delete(id: string): Promise<void>;
}

export interface ArkLinksApi {
  list(): Promise<ArkObjectLinkRecord[]>;
  upsert(link: ArkObjectLinkRecord): Promise<void>;
  delete(id: string): Promise<void>;
}

export interface ArkKvApi {
  get(key: string): Promise<string | null>;
  set(key: string, value: string): Promise<void>;
}

export type PeerConnectedCallback = (deviceId: string, deviceName: string) => void;
export type PeerDisconnectedCallback = (deviceId: string, remaining: number) => void;
export type EntityChangedCallback = (entityJson: string) => void;
