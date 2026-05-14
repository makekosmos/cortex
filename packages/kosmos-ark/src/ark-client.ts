import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

import type { KeplerLockInfo } from './ensure-kepler.js'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ArkClientOptions {
  spaceId: string
  deviceId: string
  deviceName?: string
  port?: number
  relayUrl?: string
  relayApiKey?: string
  /**
   * Optional shared secret for ARK LAN/P2P hello HMAC authentication.
   *
   * When provided, ark-core-rpc includes an HMAC proof in outbound hello
   * messages and requires the same proof from inbound peers configured with a
   * secret. This authenticates peers; it does not encrypt WebSocket traffic.
   */
  authSecret?: string
  /**
   * SQLite database path passed to ark-core-rpc during `init`.
   *
   * Required when `requestFn` is not provided. Ignored when an external
   * sidecar request function is injected because that owner is responsible
   * for initializing and switching databases.
   */
  dbPath?: string
  /**
   * Timeout for requests sent to a self-managed ark-core-rpc child process.
   *
   * Injected `requestFn` mode is not wrapped; the injected owner controls its
   * own timeout policy.
   */
  requestTimeoutMs?: number
  /**
   * Absolute path to the ark-core-rpc binary.
   *
   * When `requestFn` is provided, `sidecarPath` is ignored (the binary is
   * assumed to be already managed externally, e.g. by `sidecar.ts`).
   */
  sidecarPath?: string
  /**
   * Optional custom request function that replaces the built-in sidecar
   * child-process management.  Inject this when the calling process already
   * manages the ark-core-rpc sidecar (e.g. Electron's `sidecar.ts`) to avoid
   * spawning a second process for DB + sync.
   *
   * Signature matches `SidecarClient.request<T>(req) => Promise<T>`.
   */
  requestFn?: <T>(req: Record<string, unknown>) => Promise<T>
  /**
   * Optional function to subscribe to async sidecar events.
   * Must call the provided callback for each event object.
   * Returns an unsubscribe function.
   */
  onEventFn?: (listener: (event: SidecarEvent) => void) => (() => void)
  /**
   * Phase 2: Kepler mode. Когда задан — ArkClient не спавнит собственный
   * ark-core-rpc child, а коннектится к долгоиграющему Kepler host через
   * WebSocket. Получается из `ensureKeplerRunning()` helper.
   *
   * Wire-протокол JSON-RPC тот же что у self-managed, плюс hello-handshake
   * (token + pid + protocolVersion) на первом сообщении.
   */
  keplerLock?: KeplerLockInfo
  /** Override PID для PID-binding handshake. Default — `process.pid`. */
  keplerPidForHandshake?: number
}

interface SidecarRequest {
  operation: string
  [key: string]: unknown
}

interface SidecarResponse<T> {
  id?: string
  ok: boolean
  data?: T
  error?: string
}

export interface SidecarEvent {
  event: string
  [key: string]: unknown
}

interface PendingRequest<T> {
  id: string
  request: SidecarRequest
  resolve: (value: T) => void
  reject: (error: Error) => void
  timeout: ReturnType<typeof setTimeout>
}

export interface ConnectedPeer {
  device_id: string
  device_name: string
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

export interface ArkObjectRecord {
  id: string
  typeId: string
  title: string
  contentJson: JsonValue
  propsJson: JsonValue
  createdAt: string
  updatedAt: string
  deletedAt: string | null
}

export interface CommandManifest {
  id: string
  title: string
  subtitle?: string
  category: 'open' | 'action'
}

export interface CommandInvokedEvent {
  id: string
  params?: Record<string, unknown>
  invokerClientId?: string
}

export type CommandInvokedCallback = (event: CommandInvokedEvent) => void
export type CommandsChangedCallback = (commands: CommandManifest[]) => void

export interface ArkCommandsApi {
  register(commands: CommandManifest[]): Promise<void>
  unregister(ids: string[]): Promise<void>
  list(): Promise<CommandManifest[]>
  invoke(id: string, params?: Record<string, unknown>): Promise<void>
  /** Подписаться на событие command_invoked. Возвращает unsubscribe-функцию. */
  onInvoked(handler: CommandInvokedCallback): () => void
  /** Подписаться на event commands_changed (изменился список доступных). */
  onChanged(handler: CommandsChangedCallback): () => void
}

export interface ArkObjectTypeRecord {
  id: string
  name: string
  schemaJson: string
  uiSchemaJson: string
  createdAt: string
  updatedAt: string
  systemLocked: boolean
}

export interface ArkObjectLinkRecord {
  id: string
  sourceObjectId: string
  targetObjectId: string
  linkType: string
  createdAt: string
}

export interface ArkSearchResult {
  file: string
  line: number
  text: string
  entryId: string
}

export interface ArkTrackedAppRecord {
  id: string
  platform: string
  exePath: string
  normalizedExePath: string
  processName: string
  displayName: string | null
  publisher: string | null
  iconRef: string | null
  firstSeenAt: string
  lastSeenAt: string
}

export interface ArkUsageSessionRecord {
  id: string
  trackedAppId: string
  deviceId: string
  deviceName: string
  platform: string
  startedAt: string
  endedAt: string | null
  foregroundMs: number
  idleMs: number
  windowTitle: string | null
  processName: string
  exePath: string
  pidStart: number | null
  pidEnd: number | null
  metaJson: JsonValue
}

export interface ArkUsageEventRecord {
  id: string
  trackedAppId: string
  usageSessionId: string | null
  deviceId: string
  deviceName: string
  platform: string
  occurredAt: string
  kind: string
  windowTitle: string | null
  processName: string
  exePath: string
  pid: number | null
  isForeground: boolean
  isIdle: boolean
  metaJson: JsonValue
}

export interface ArkUsageSnapshot {
  trackedApps: ArkTrackedAppRecord[]
  usageSessions: ArkUsageSessionRecord[]
  usageEvents: ArkUsageEventRecord[]
}

export interface ArkUsageAnalyticsOptions {
  rangeDays?: number
  topAppsLimit?: number
  recentSessionsLimit?: number
}

export interface ArkUsageSummary {
  trackedAppCount: number
  sessionCount: number
  eventCount: number
  totalForegroundMs: number
  totalIdleMs: number
  firstRecordedAt: string | null
  lastRecordedAt: string | null
}

export interface ArkDailyTrendPoint {
  date: string
  foregroundMs: number
  idleMs: number
  sessions: number
}

export interface ArkHourlyHeatmapCell {
  weekday: number
  hour: number
  foregroundMs: number
}

export interface ArkTopAppEntry {
  id: string
  displayName: string
  processName: string
  normalizedPath: string
  foregroundMs: number
  idleMs: number
  sessions: number
  lastSeenAt: string | null
}

export interface ArkRecentSessionEntry {
  id: string
  trackedAppId: string
  displayName: string
  processName: string
  platform: string
  deviceName: string
  startedAt: string
  endedAt: string | null
  foregroundMs: number
  idleMs: number
  windowTitle: string | null
}

export interface ArkUsageProcessCandidate {
  trackedAppId: string
  displayName: string
  exePath: string | null
  processName: string | null
  lastSeenAt: string | null
  sessionCount: number
  bindingMatchType: 'exe_path' | 'process_name'
  bindingMatchValue: string
  bindingNormalizedValue: string
}

export interface ArkUsageGamePlaytimeBinding {
  gameId: string
  gameName: string
  matchType: 'exe_path' | 'process_name'
  matchValue: string
}

export interface ArkUsageGamePlaytimeAggregate {
  gameId: string
  gameName: string
  totalSeconds: number
  sessionCount: number
  lastPlayed: string | null
}

export interface ArkUsageGameDailyTotal {
  date: string
  seconds: number
}

export interface ArkUsageGameRangeTotal {
  gameId: string
  gameName: string
  seconds: number
}

export interface ArkUsageGamePlaytimeSummaryOptions {
  bindings: ArkUsageGamePlaytimeBinding[]
  rangeStart?: string
  rangeEnd?: string
}

export interface ArkUsageGamePlaytimeSummary {
  aggregates: ArkUsageGamePlaytimeAggregate[]
  dailyTotals: ArkUsageGameDailyTotal[]
  perGameTotals: ArkUsageGameRangeTotal[]
}

export interface ArkUsageAnalyticsSnapshot {
  generatedAt: string
  summary: ArkUsageSummary
  dailyTrend: ArkDailyTrendPoint[]
  hourlyHeatmap: ArkHourlyHeatmapCell[]
  topApps: ArkTopAppEntry[]
  recentSessions: ArkRecentSessionEntry[]
}

export interface ArkUsageEntityApi<T> {
  upsert(record: T): Promise<void>
  delete(id: string): Promise<void>
}

export interface ArkUsageApi {
  loadAll(): Promise<ArkUsageSnapshot>
  analytics: {
    snapshot(options?: ArkUsageAnalyticsOptions): Promise<ArkUsageAnalyticsSnapshot>
  }
  processes: {
    recent(limit?: number): Promise<ArkUsageProcessCandidate[]>
    search(query: string, limit?: number): Promise<ArkUsageProcessCandidate[]>
  }
  gamePlaytime: {
    summary(options: ArkUsageGamePlaytimeSummaryOptions): Promise<ArkUsageGamePlaytimeSummary>
  }
  trackedApps: ArkUsageEntityApi<ArkTrackedAppRecord>
  sessions: ArkUsageEntityApi<ArkUsageSessionRecord>
  events: ArkUsageEntityApi<ArkUsageEventRecord>
}

export interface ArkObjectsApi {
  list(): Promise<ArkObjectRecord[]>
  listByType(typeId: string): Promise<ArkObjectRecord[]>
  get(id: string): Promise<ArkObjectRecord | null>
  getMany(ids: readonly string[]): Promise<ArkObjectRecord[]>
  upsert(object: ArkObjectRecord): Promise<void>
  delete(id: string): Promise<void>
  search(query: string): Promise<ArkSearchResult[]>
}

export interface ArkObjectTypesApi {
  list(): Promise<ArkObjectTypeRecord[]>
  get(id: string): Promise<ArkObjectTypeRecord | null>
  upsert(objectType: ArkObjectTypeRecord): Promise<void>
  delete(id: string): Promise<void>
}

export interface ArkLinksApi {
  list(): Promise<ArkObjectLinkRecord[]>
  upsert(link: ArkObjectLinkRecord): Promise<void>
  delete(id: string): Promise<void>
}

export interface ArkKvApi {
  get(key: string): Promise<string | null>
  set(key: string, value: string): Promise<void>
}

interface ArkLoadAllData {
  trackedApps?: ArkTrackedAppRecord[]
  usageSessions?: ArkUsageSessionRecord[]
  usageEvents?: ArkUsageEventRecord[]
}

export type PeerConnectedCallback = (deviceId: string, deviceName: string) => void
export type PeerDisconnectedCallback = (deviceId: string, remaining: number) => void
export type EntityChangedCallback = (entityJson: string) => void

// ---------------------------------------------------------------------------
// ArkClient
// ---------------------------------------------------------------------------

const MAX_QUEUE_SIZE = 500
const DEFAULT_REQUEST_TIMEOUT_MS = 30_000

export class ArkClient {
  private readonly opts: ArkClientOptions
  readonly objects: ArkObjectsApi
  readonly objectTypes: ArkObjectTypesApi
  readonly links: ArkLinksApi
  readonly usage: ArkUsageApi
  readonly kv: ArkKvApi
  readonly commands: ArkCommandsApi

  // ---- Built-in sidecar child-process (used when requestFn is absent) ----
  private child: ChildProcessWithoutNullStreams | null = null
  private stdoutChunks: Buffer[] = []
  private stdoutLength = 0
  private stderrBuffer = ''
  private pendingRequests: Map<string, PendingRequest<unknown>> = new Map()
  private nextRequestSeq = 0
  private initialized = false

  // ---- Injected sidecar delegate ----
  private readonly delegateRequest: (<T>(req: Record<string, unknown>) => Promise<T>) | null
  private delegateUnsubscribe: (() => void) | null = null

  // ---- Kepler mode (Phase 2) ----
  private keplerWs: WebSocket | null = null
  private keplerHandshakeDone = false
  private keplerConnectPromise: Promise<void> | null = null

  // ---- Event callbacks ----
  private peerConnectedCallbacks: Set<PeerConnectedCallback> = new Set()
  private peerDisconnectedCallbacks: Set<PeerDisconnectedCallback> = new Set()
  private entityChangedCallbacks: Set<EntityChangedCallback> = new Set()
  private arkEventCallbacks: Set<(event: SidecarEvent) => void> = new Set()
  private commandInvokedCallbacks: Set<CommandInvokedCallback> = new Set()
  private commandsChangedCallbacks: Set<CommandsChangedCallback> = new Set()

  constructor(opts: ArkClientOptions) {
    this.opts = opts
    this.delegateRequest = opts.requestFn ?? null
    this.objects = {
      list: () => this.requestAfterInit<ArkObjectRecord[]>({ operation: 'list_objects' }),
      listByType: (typeId) =>
        this.requestAfterInit<ArkObjectRecord[]>({ operation: 'list_objects_by_type', type_id: typeId }),
      get: (id) => this.requestAfterInit<ArkObjectRecord | null>({ operation: 'get_object', id }),
      getMany: (ids) =>
        this.requestAfterInit<ArkObjectRecord[]>({ operation: 'get_objects_by_ids', ids: [...ids] }),
      upsert: async (object) => {
        await this.requestAfterInit<boolean>({
          operation: 'upsert_object',
          object,
          device_id: this.opts.deviceId,
        })
      },
      delete: async (id) => {
        await this.requestAfterInit<boolean>({
          operation: 'delete_object',
          id,
          device_id: this.opts.deviceId,
        })
      },
      search: (query) => this.requestAfterInit<ArkSearchResult[]>({ operation: 'search_objects', query }),
    }
    this.objectTypes = {
      list: () => this.requestAfterInit<ArkObjectTypeRecord[]>({ operation: 'list_object_types' }),
      get: (id) => this.requestAfterInit<ArkObjectTypeRecord | null>({ operation: 'get_object_type', id }),
      upsert: async (objectType) => {
        await this.requestAfterInit<boolean>({
          operation: 'upsert_object_type',
          object_type: objectType,
          device_id: this.opts.deviceId,
        })
      },
      delete: async (id) => {
        await this.requestAfterInit<boolean>({
          operation: 'delete_object_type',
          id,
          device_id: this.opts.deviceId,
        })
      },
    }
    this.links = {
      list: () => this.requestAfterInit<ArkObjectLinkRecord[]>({ operation: 'list_object_links' }),
      upsert: async (link) => {
        await this.requestAfterInit<boolean>({
          operation: 'upsert_object_link',
          object_link: link,
          device_id: this.opts.deviceId,
        })
      },
      delete: async (id) => {
        await this.requestAfterInit<boolean>({
          operation: 'delete_object_link',
          id,
          device_id: this.opts.deviceId,
        })
      },
    }
    this.kv = {
      get: (key) => this.requestAfterInit<string | null>({ operation: 'get_sync_kv', key }),
      set: async (key, value) => {
        await this.requestAfterInit<boolean>({ operation: 'set_sync_kv', key, value })
      },
    }
    this.usage = {
      loadAll: async () => {
        const data = await this.requestAfterInit<ArkLoadAllData>({ operation: 'load_all' })
        return {
          trackedApps: data.trackedApps ?? [],
          usageSessions: data.usageSessions ?? [],
          usageEvents: data.usageEvents ?? [],
        }
      },
      analytics: {
        snapshot: (options = {}) =>
          this.requestAfterInit<ArkUsageAnalyticsSnapshot>({
            operation: 'get_usage_analytics',
            range_days: options.rangeDays,
            top_apps_limit: options.topAppsLimit,
            recent_sessions_limit: options.recentSessionsLimit,
          }),
      },
      processes: {
        recent: (limit = 10) =>
          this.requestAfterInit<ArkUsageProcessCandidate[]>({
            operation: 'list_recent_usage_processes',
            limit,
          }),
        search: (query, limit = 10) =>
          this.requestAfterInit<ArkUsageProcessCandidate[]>({
            operation: 'search_usage_processes',
            query,
            limit,
          }),
      },
      gamePlaytime: {
        summary: (options) =>
          this.requestAfterInit<ArkUsageGamePlaytimeSummary>({
            operation: 'get_usage_game_playtime_summary',
            bindings: options.bindings,
            range_start: options.rangeStart,
            range_end: options.rangeEnd,
          }),
      },
      trackedApps: {
        upsert: async (record) => {
          await this.requestAfterInit<boolean>({
            operation: 'upsert_tracked_app',
            tracked_app: record,
            device_id: this.opts.deviceId,
          })
        },
        delete: async (id) => {
          await this.requestAfterInit<boolean>({
            operation: 'delete_tracked_app',
            id,
            device_id: this.opts.deviceId,
          })
        },
      },
      sessions: {
        upsert: async (record) => {
          await this.requestAfterInit<boolean>({
            operation: 'upsert_usage_session',
            usage_session: record,
            device_id: this.opts.deviceId,
          })
        },
        delete: async (id) => {
          await this.requestAfterInit<boolean>({
            operation: 'delete_usage_session',
            id,
            device_id: this.opts.deviceId,
          })
        },
      },
      events: {
        upsert: async (record) => {
          await this.requestAfterInit<boolean>({
            operation: 'upsert_usage_event',
            usage_event: record,
            device_id: this.opts.deviceId,
          })
        },
        delete: async (id) => {
          await this.requestAfterInit<boolean>({
            operation: 'delete_usage_event',
            id,
            device_id: this.opts.deviceId,
          })
        },
      },
    }
    this.commands = {
      register: async (commands) => {
        await this.requestAfterInit<boolean>({
          operation: 'commands.register',
          commands,
        })
      },
      unregister: async (ids) => {
        await this.requestAfterInit<boolean>({
          operation: 'commands.unregister',
          ids,
        })
      },
      list: async () => {
        const result = await this.requestAfterInit<{ commands: CommandManifest[] }>({
          operation: 'commands.list',
        })
        return result.commands
      },
      invoke: async (id, params) => {
        await this.requestAfterInit<boolean>({
          operation: 'commands.invoke',
          id,
          ...(params !== undefined ? { params } : {}),
        })
      },
      onInvoked: (handler) => {
        this.commandInvokedCallbacks.add(handler)
        return () => { this.commandInvokedCallbacks.delete(handler) }
      },
      onChanged: (handler) => {
        this.commandsChangedCallbacks.add(handler)
        return () => { this.commandsChangedCallbacks.delete(handler) }
      },
    }
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  async start(): Promise<void> {
    await this.ensureInitialized()

    // Subscribe to events from the injected delegate or from our own child.
    if (this.opts.onEventFn) {
      this.delegateUnsubscribe = this.opts.onEventFn((event) => {
        this.dispatchSidecarEvent(event)
      })
    }

    // Architectural change: sync — opt-in feature, активируется только когда
    // запущен Kepler host (Kepler сам вызывает start_sync). Standalone-apps
    // (без Kepler installed) работают как локальные DB clients без LAN/relay
    // sync. Это позволяет single-machine usage без overhead Kepler installer'а.
    //
    // Apps что explicitly хотят standalone sync (rare case — например, для
    // тестов или специальных deployments) — могут вызвать `startSyncLegacy()`
    // напрямую.
  }

  /**
   * Legacy explicit `start_sync` для apps, которые хотят запустить sync без
   * Kepler host'а. В обычном flow — Kepler владеет sync, apps его не дёргают.
   */
  async startSyncLegacy(): Promise<void> {
    if (this.opts.keplerLock) {
      // Kepler уже владеет sync — повторный start_sync только нарушит state.
      return
    }
    await this.request<boolean>({
      operation: 'start_sync',
      space_id: this.opts.spaceId,
      device_id: this.opts.deviceId,
      device_name: this.opts.deviceName ?? null,
      port: this.opts.port ?? null,
      seed_addresses: null,
      relay_url: this.opts.relayUrl ?? null,
      relay_api_key: this.opts.relayApiKey ?? null,
      auth_secret: this.opts.authSecret ?? null,
    })
  }

  async stop(): Promise<void> {
    try {
      if (!this.opts.keplerLock) {
        // В kepler-режиме stop_sync владеет Kepler, апка не имеет права его дёргать.
        await this.request<boolean>({ operation: 'stop_sync' })
      }
    } catch {
      // Ignore errors on stop
    }
    this.delegateUnsubscribe?.()
    this.delegateUnsubscribe = null
    if (this.opts.keplerLock) {
      this.closeKeplerConnection()
    } else if (!this.delegateRequest) {
      this.killChild()
    }
  }

  // -------------------------------------------------------------------------
  // Sync operations
  // -------------------------------------------------------------------------

  async broadcastChange(
    entityType: string,
    entityId: string,
    data: Record<string, unknown>,
    deleted = false,
  ): Promise<void> {
    await this.request<boolean>({
      operation: 'broadcast_change',
      entity: {
        type: entityType,
        id: entityId,
        data,
        hlc: this.generateHlc(),
        ...(deleted ? { deleted: true } : {}),
      },
    })
  }

  async getConnectedPeers(): Promise<Array<{ device_id: string; device_name: string }>> {
    return this.request<ConnectedPeer[]>({ operation: 'get_connected_peers' })
  }

  /**
   * Public escape-hatch: прямой проход в ARK ops без typed wrapper. Используется
   * legacy callsites (Eden's main/store.ts) на время Phase 2 cutover — там
   * сотни вызовов через `runArkRequest({operation: "...", ...})`, переписывать
   * всё на typed API одновременно с переключением транспорта — слишком большой
   * blast radius. Когда Eden постепенно мигрирует на `client.objects.list()`
   * etc. — этот метод можно будет deprecate.
   *
   * Гарантирует, что connection (kepler или self-managed child) инициализирован.
   */
  async invokeOperation<T>(req: { operation: string; [key: string]: unknown }): Promise<T> {
    return this.requestAfterInit<T>(req as SidecarRequest)
  }

  // -------------------------------------------------------------------------
  // Event subscriptions (return unsubscribe function)
  // -------------------------------------------------------------------------

  onPeerConnected(cb: PeerConnectedCallback): () => void {
    this.peerConnectedCallbacks.add(cb)
    return () => { this.peerConnectedCallbacks.delete(cb) }
  }

  onPeerDisconnected(cb: PeerDisconnectedCallback): () => void {
    this.peerDisconnectedCallbacks.add(cb)
    return () => { this.peerDisconnectedCallbacks.delete(cb) }
  }

  onEntityChanged(cb: EntityChangedCallback): () => void {
    this.entityChangedCallbacks.add(cb)
    return () => { this.entityChangedCallbacks.delete(cb) }
  }

  /**
   * Generic event subscription — получает все события от ark-core-rpc (любого
   * `event` field). Используется для Phase 2 sync_error / sync_replay events,
   * у которых нет типизированного callback'а. Apps подписываются и фильтруют:
   *
   *   client.onArkEvent(e => {
   *     if (e.event === "sync_error") { ... }
   *   })
   *
   * Возвращает unsubscribe.
   */
  onArkEvent(cb: (event: SidecarEvent) => void): () => void {
    this.arkEventCallbacks.add(cb)
    return () => { this.arkEventCallbacks.delete(cb) }
  }

  // -------------------------------------------------------------------------
  // Internal: request routing
  // -------------------------------------------------------------------------

  private request<T>(req: SidecarRequest): Promise<T> {
    if (this.delegateRequest) {
      // Use the injected request function (external sidecar management).
      return this.delegateRequest<T>(req as Record<string, unknown>)
    }
    if (this.opts.keplerLock) {
      // Phase 2: Kepler mode — WS к долгоиграющему Kepler host.
      return this.requestViaKepler<T>(req)
    }
    // Use built-in child-process sidecar.
    return this.requestViaChild<T>(req)
  }

  private async requestAfterInit<T>(req: SidecarRequest): Promise<T> {
    await this.ensureInitialized()
    return this.request<T>(req)
  }

  // -------------------------------------------------------------------------
  // Internal: sidecar child-process management
  // -------------------------------------------------------------------------

  private ensureChild(): ChildProcessWithoutNullStreams {
    if (this.child) return this.child

    const binaryPath = this.opts.sidecarPath
    if (!binaryPath) throw new Error('@kosmos/ark: sidecarPath is required when requestFn is not provided')

    const child = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    })

    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk: string) => { this.stderrBuffer += chunk })

    child.stdout.on('data', (chunk: Buffer | string) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
      this.stdoutChunks.push(buf)
      this.stdoutLength += buf.length
      this.flushStdout()
    })

    child.on('error', (err) => {
      this.failAll(err instanceof Error ? err : new Error(String(err)))
    })

    child.on('close', (code) => {
      const reason = this.stderrBuffer.trim() || `ark-core-rpc exited with ${code}`
      this.failAll(new Error(reason))
    })

    this.child = child
    this.stdoutChunks = []
    this.stdoutLength = 0
    this.stderrBuffer = ''

    return child
  }

  private killChild(): void {
    if (this.child) {
      this.child.stdout.removeAllListeners()
      this.child.stderr.removeAllListeners()
      this.child.removeAllListeners()
      if (!this.child.killed) this.child.kill()
      this.child = null
    }
    this.stdoutChunks = []
    this.stdoutLength = 0
    this.stderrBuffer = ''
    for (const pending of this.pendingRequests.values()) {
      clearTimeout(pending.timeout)
    }
    this.pendingRequests.clear()
    this.initialized = false
  }

  private flushStdout(): void {
    const merged = Buffer.concat(this.stdoutChunks, this.stdoutLength)
    this.stdoutChunks = []
    this.stdoutLength = 0

    let searchFrom = 0
    while (true) {
      const nl = merged.indexOf(0x0a, searchFrom)
      if (nl === -1) {
        if (searchFrom < merged.length) {
          const remaining = merged.subarray(searchFrom)
          this.stdoutChunks.push(remaining)
          this.stdoutLength = remaining.length
        }
        return
      }

      const rawLine = merged.subarray(searchFrom, nl).toString('utf8').trim()
      searchFrom = nl + 1

      if (!rawLine) continue

      let parsed: unknown
      try {
        parsed = JSON.parse(rawLine)
      } catch (err) {
        this.failAll(err instanceof Error ? err : new Error(String(err)))
        continue
      }

      const obj = parsed as Record<string, unknown>

      // Async events have `event` field, no `ok` field.
      if ('event' in obj && !('ok' in obj)) {
        this.dispatchSidecarEvent(obj as unknown as SidecarEvent)
        continue
      }

      const resp = obj as unknown as SidecarResponse<unknown>
      // Rust echoes envelope-id обратно как `_req_id` (новое) или `id` (legacy).
      const respReqId =
        typeof obj._req_id === "string"
          ? (obj._req_id as string)
          : typeof obj.id === "string"
            ? (obj.id as string)
            : undefined
      const pending = this.takePendingRequest(respReqId)
      if (!pending) continue

      if (!resp.ok) {
        pending.reject(new Error(resp.error ?? 'ark-core-rpc error'))
      } else {
        pending.resolve(resp.data)
      }
    }
  }

  private takePendingRequest(id: string | undefined): PendingRequest<unknown> | null {
    if (id) {
      const pending = this.pendingRequests.get(id)
      if (!pending) return null
      this.pendingRequests.delete(id)
      clearTimeout(pending.timeout)
      return pending
    }

    // Backward-compatible fallback for old sidecars that do not echo ids.
    const first = this.pendingRequests.keys().next()
    if (first.done) return null
    const fallbackId = first.value
    const pending = this.pendingRequests.get(fallbackId) ?? null
    this.pendingRequests.delete(fallbackId)
    if (pending) clearTimeout(pending.timeout)
    return pending
  }

  private makeRequestId(): string {
    this.nextRequestSeq += 1
    return `kosmos-ark-${Date.now()}-${this.nextRequestSeq}`
  }

  private sendRequest<T>(req: SidecarRequest): Promise<T> {
    if (this.pendingRequests.size >= MAX_QUEUE_SIZE) {
      return Promise.reject(new Error(`@kosmos/ark request queue overflow (${MAX_QUEUE_SIZE})`))
    }

    const child = this.ensureChild()
    const id = this.makeRequestId()
    // ВАЖНО: envelope-id живёт в `_req_id`, не в `id`. Иначе он бы затирал
    // payload-поле `id` у операций вроде get_object / delete_object — это
    // приводит к молчаливому "not found", потому что Rust пытался искать
    // строку запроса в БД. Rust обрабатывает оба поля для обратной совместимости.
    const request = { ...req, _req_id: id }
    const timeoutMs = this.opts.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS

    const promise = new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const pending = this.pendingRequests.get(id)
        if (!pending) return
        this.pendingRequests.delete(id)
        reject(new Error(`@kosmos/ark request timed out after ${timeoutMs}ms: ${req.operation}`))
      }, timeoutMs)
      this.pendingRequests.set(id, {
        id,
        request,
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      })
    })

    try {
      child.stdin.write(`${JSON.stringify(request)}\n`)
    } catch (err) {
      const pending = this.pendingRequests.get(id)
      if (pending) {
        clearTimeout(pending.timeout)
        this.pendingRequests.delete(id)
      }
      this.killChild()
      return Promise.reject(err instanceof Error ? err : new Error(String(err)))
    }

    return promise
  }

  private failAll(error: Error): void {
    const pending = Array.from(this.pendingRequests.values())
    this.pendingRequests.clear()
    this.child = null
    pending.forEach(r => {
      clearTimeout(r.timeout)
      r.reject(error)
    })
  }

  private requestViaChild<T>(req: SidecarRequest): Promise<T> {
    return this.sendRequest<T>(req)
  }

  private async ensureInitialized(): Promise<void> {
    if (this.delegateRequest || this.initialized) return

    if (this.opts.keplerLock) {
      // Kepler host уже init'нул ark-core-rpc — нам не нужно слать init.
      // Просто гарантируем что WS-handshake завершён, после чего готовы.
      await this.ensureKeplerConnection()
      this.initialized = true
      return
    }

    const dbPath = this.opts.dbPath
    if (!dbPath) {
      throw new Error('@kosmos/ark: dbPath is required when requestFn is not provided')
    }

    fs.mkdirSync(path.dirname(dbPath), { recursive: true })
    await this.requestViaChild<boolean>({ operation: 'init', dbPath })
    this.initialized = true
  }

  private dispatchSidecarEvent(event: SidecarEvent): void {
    // Generic subscribers first — получают ВСЕ события, в том числе те, у которых
    // нет типизированного callback'а (например, `sync_error`, `sync_replay`).
    for (const cb of this.arkEventCallbacks) {
      try { cb(event) } catch { /* ignore */ }
    }
    switch (event.event) {
      case 'entity_changed': {
        const entityJson = typeof event.entity === 'object'
          ? JSON.stringify(event.entity)
          : String(event.entity ?? '')
        for (const cb of this.entityChangedCallbacks) {
          try { cb(entityJson) } catch { /* ignore */ }
        }
        break
      }
      case 'peer_connected': {
        const deviceId = String(event.device_id ?? '')
        const deviceName = String(event.device_name ?? '')
        for (const cb of this.peerConnectedCallbacks) {
          try { cb(deviceId, deviceName) } catch { /* ignore */ }
        }
        break
      }
      case 'peer_disconnected': {
        const deviceId = String(event.device_id ?? '')
        const remaining = typeof event.remaining === 'number' ? event.remaining : 0
        for (const cb of this.peerDisconnectedCallbacks) {
          try { cb(deviceId, remaining) } catch { /* ignore */ }
        }
        break
      }
      case 'command_invoked': {
        const id = String(event.id ?? '')
        const rawParams = event.params
        const params =
          rawParams && typeof rawParams === 'object' && !Array.isArray(rawParams)
            ? (rawParams as Record<string, unknown>)
            : undefined
        const invokerClientId =
          typeof event.invokerClientId === 'string'
            ? event.invokerClientId
            : typeof event.invoker_client_id === 'string'
              ? (event.invoker_client_id as string)
              : undefined
        const payload: CommandInvokedEvent = {
          id,
          ...(params !== undefined ? { params } : {}),
          ...(invokerClientId !== undefined ? { invokerClientId } : {}),
        }
        for (const cb of this.commandInvokedCallbacks) {
          try { cb(payload) } catch { /* ignore */ }
        }
        break
      }
      case 'commands_changed': {
        const list = Array.isArray(event.commands) ? (event.commands as CommandManifest[]) : []
        for (const cb of this.commandsChangedCallbacks) {
          try { cb(list) } catch { /* ignore */ }
        }
        break
      }
      default:
        break
    }
  }

  private generateHlc(): string {
    const now = new Date().toISOString()
    const counter = String(Math.floor(Math.random() * 999999)).padStart(6, '0')
    return `${now}:${counter}:${this.opts.deviceId}`
  }

  // -------------------------------------------------------------------------
  // Internal: Kepler mode (WebSocket → Kepler host)
  // -------------------------------------------------------------------------

  private async ensureKeplerConnection(): Promise<void> {
    if (
      this.keplerHandshakeDone &&
      this.keplerWs &&
      this.keplerWs.readyState === WebSocket.OPEN
    ) {
      return
    }
    if (this.keplerConnectPromise) {
      return this.keplerConnectPromise
    }
    this.keplerConnectPromise = this.openKeplerConnection()
    try {
      await this.keplerConnectPromise
    } finally {
      this.keplerConnectPromise = null
    }
  }

  private async openKeplerConnection(): Promise<void> {
    const lock = this.opts.keplerLock
    if (!lock) {
      throw new Error('@kosmos/ark: keplerLock is required for kepler mode')
    }

    const url = `ws://127.0.0.1:${lock.ws_port}`
    const ws = new WebSocket(url)

    await new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        ws.removeEventListener('error', onError as never)
        resolve()
      }
      const onError = (_e: Event) => {
        ws.removeEventListener('open', onOpen as never)
        reject(new Error(`Kepler WS open failed at ${url}`))
      }
      ws.addEventListener('open', onOpen, { once: true })
      ws.addEventListener('error', onError, { once: true })
    })

    const protocolVersionString = `${lock.protocol_version.major}.${lock.protocol_version.minor}.${lock.protocol_version.patch}`
    const helloPayload = {
      kind: 'hello',
      protocolVersion: protocolVersionString,
      token: lock.auth_token,
      pid: this.opts.keplerPidForHandshake ?? process.pid,
      clientId: this.opts.deviceId,
    }
    ws.send(JSON.stringify(helloPayload))

    const helloResponse = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const onMessage = (evt: MessageEvent) => {
        ws.removeEventListener('message', onMessage as never)
        try {
          resolve(JSON.parse(String(evt.data)) as Record<string, unknown>)
        } catch (e) {
          reject(e instanceof Error ? e : new Error(String(e)))
        }
      }
      ws.addEventListener('message', onMessage, { once: true })
    })

    if (helloResponse.kind !== 'hello_ok') {
      ws.close()
      const code = String(helloResponse.code ?? 'unknown')
      const message = String(helloResponse.message ?? '')
      throw new Error(`Kepler rejected handshake (${code}): ${message}`)
    }

    // Ongoing message handler — обрабатывает все frames после hello_ok.
    ws.addEventListener('message', (evt) => {
      this.handleKeplerFrame(String(evt.data))
    })
    ws.addEventListener('close', () => {
      this.keplerWs = null
      this.keplerHandshakeDone = false
      // Pending requests fail с явной ошибкой; апка должна implement reconnect
      // через ensureKeplerRunning + новый ArkClient. AC2 на этом уровне.
      const error = new Error('Kepler connection closed')
      for (const pending of this.pendingRequests.values()) {
        clearTimeout(pending.timeout)
        pending.reject(error)
      }
      this.pendingRequests.clear()
    })
    ws.addEventListener('error', () => {
      // 'close' followed обычно — single handler. Здесь — просто log/noop.
    })

    this.keplerWs = ws
    this.keplerHandshakeDone = true
  }

  private handleKeplerFrame(text: string): void {
    if (!text.trim()) return
    let parsed: unknown
    try {
      parsed = JSON.parse(text)
    } catch {
      return
    }
    const obj = parsed as Record<string, unknown>

    // Async events (no `ok`).
    if ('event' in obj && !('ok' in obj)) {
      this.dispatchSidecarEvent(obj as unknown as SidecarEvent)
      return
    }

    const resp = obj as unknown as SidecarResponse<unknown>
    const respReqId =
      typeof obj._req_id === 'string'
        ? (obj._req_id as string)
        : typeof obj.id === 'string'
          ? (obj.id as string)
          : undefined
    const pending = this.takePendingRequest(respReqId)
    if (!pending) return

    if (!resp.ok) {
      pending.reject(new Error(resp.error ?? 'kepler error'))
    } else {
      pending.resolve(resp.data)
    }
  }

  private async requestViaKepler<T>(req: SidecarRequest): Promise<T> {
    await this.ensureKeplerConnection()
    const ws = this.keplerWs
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      throw new Error('Kepler WS not connected')
    }
    if (this.pendingRequests.size >= MAX_QUEUE_SIZE) {
      return Promise.reject(
        new Error(`@kosmos/ark request queue overflow (${MAX_QUEUE_SIZE})`),
      )
    }

    const id = this.makeRequestId()
    const request = { ...req, _req_id: id }
    const timeoutMs = this.opts.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS

    const promise = new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const pending = this.pendingRequests.get(id)
        if (!pending) return
        this.pendingRequests.delete(id)
        reject(
          new Error(`@kosmos/ark request timed out after ${timeoutMs}ms: ${req.operation}`),
        )
      }, timeoutMs)
      this.pendingRequests.set(id, {
        id,
        request,
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      })
    })

    try {
      ws.send(JSON.stringify(request))
    } catch (err) {
      const pending = this.pendingRequests.get(id)
      if (pending) {
        clearTimeout(pending.timeout)
        this.pendingRequests.delete(id)
      }
      return Promise.reject(err instanceof Error ? err : new Error(String(err)))
    }

    return promise
  }

  private closeKeplerConnection(): void {
    if (this.keplerWs) {
      try {
        this.keplerWs.close()
      } catch {
        // ignore
      }
      this.keplerWs = null
    }
    this.keplerHandshakeDone = false
    for (const pending of this.pendingRequests.values()) {
      clearTimeout(pending.timeout)
    }
    this.pendingRequests.clear()
  }
}
