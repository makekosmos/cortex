import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'

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
}

interface SidecarRequest {
  operation: string
  [key: string]: unknown
}

interface SidecarResponse<T> {
  ok: boolean
  data?: T
  error?: string
}

export interface SidecarEvent {
  event: string
  [key: string]: unknown
}

interface PendingRequest<T> {
  request: SidecarRequest
  resolve: (value: T) => void
  reject: (error: Error) => void
}

export interface ConnectedPeer {
  device_id: string
  device_name: string
}

export type PeerConnectedCallback = (deviceId: string, deviceName: string) => void
export type PeerDisconnectedCallback = (deviceId: string, remaining: number) => void
export type EntityChangedCallback = (entityJson: string) => void

// ---------------------------------------------------------------------------
// ArkClient
// ---------------------------------------------------------------------------

const MAX_QUEUE_SIZE = 500

export class ArkClient {
  private readonly opts: ArkClientOptions

  // ---- Built-in sidecar child-process (used when requestFn is absent) ----
  private child: ChildProcessWithoutNullStreams | null = null
  private stdoutChunks: Buffer[] = []
  private stdoutLength = 0
  private stderrBuffer = ''
  private requestQueue: Array<PendingRequest<unknown>> = []
  private activeRequest: PendingRequest<unknown> | null = null

  // ---- Injected sidecar delegate ----
  private readonly delegateRequest: (<T>(req: Record<string, unknown>) => Promise<T>) | null
  private delegateUnsubscribe: (() => void) | null = null

  // ---- Event callbacks ----
  private peerConnectedCallbacks: Set<PeerConnectedCallback> = new Set()
  private peerDisconnectedCallbacks: Set<PeerDisconnectedCallback> = new Set()
  private entityChangedCallbacks: Set<EntityChangedCallback> = new Set()

  constructor(opts: ArkClientOptions) {
    this.opts = opts
    this.delegateRequest = opts.requestFn ?? null
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  async start(): Promise<void> {
    // Subscribe to events from the injected delegate or from our own child.
    if (this.opts.onEventFn) {
      this.delegateUnsubscribe = this.opts.onEventFn((event) => {
        this.dispatchSidecarEvent(event)
      })
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
    })
  }

  async stop(): Promise<void> {
    try {
      await this.request<boolean>({ operation: 'stop_sync' })
    } catch {
      // Ignore errors on stop
    }
    this.delegateUnsubscribe?.()
    this.delegateUnsubscribe = null
    if (!this.delegateRequest) {
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

  // -------------------------------------------------------------------------
  // Internal: request routing
  // -------------------------------------------------------------------------

  private request<T>(req: SidecarRequest): Promise<T> {
    if (this.delegateRequest) {
      // Use the injected request function (external sidecar management).
      return this.delegateRequest<T>(req as Record<string, unknown>)
    }
    // Use built-in child-process sidecar.
    return this.requestViaChild<T>(req)
  }

  // -------------------------------------------------------------------------
  // Internal: sidecar child-process management
  // -------------------------------------------------------------------------

  private ensureChild(): ChildProcessWithoutNullStreams {
    if (this.child) return this.child

    const binaryPath = this.opts.sidecarPath
    if (!binaryPath) throw new Error('@arksync/node: sidecarPath is required when requestFn is not provided')

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
    this.activeRequest = null
    this.requestQueue = []
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
        const pending = this.activeRequest
        this.activeRequest = null
        if (pending) {
          pending.reject(err instanceof Error ? err : new Error(String(err)))
          this.dispatchNext()
        }
        continue
      }

      const obj = parsed as Record<string, unknown>

      // Async events have `event` field, no `ok` field.
      if ('event' in obj && !('ok' in obj)) {
        this.dispatchSidecarEvent(obj as unknown as SidecarEvent)
        continue
      }

      const pending = this.activeRequest
      this.activeRequest = null
      if (!pending) continue

      const resp = obj as unknown as SidecarResponse<unknown>
      if (!resp.ok) {
        pending.reject(new Error(resp.error ?? 'ark-core-rpc error'))
      } else {
        pending.resolve(resp.data)
      }
      this.dispatchNext()
    }
  }

  private dispatchNext(): void {
    if (this.activeRequest || this.requestQueue.length === 0) return
    const next = this.requestQueue.shift()
    if (!next) return
    const child = this.ensureChild()
    this.activeRequest = next
    try {
      child.stdin.write(`${JSON.stringify(next.request)}\n`)
    } catch (err) {
      this.activeRequest = null
      next.reject(err instanceof Error ? err : new Error(String(err)))
      this.killChild()
      this.dispatchNext()
    }
  }

  private failAll(error: Error): void {
    const pending = this.activeRequest
    const queued = this.requestQueue.splice(0)
    this.activeRequest = null
    this.child = null
    if (pending) pending.reject(error)
    queued.forEach(r => { r.reject(error) })
  }

  private requestViaChild<T>(req: SidecarRequest): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (this.requestQueue.length >= MAX_QUEUE_SIZE) {
        reject(new Error(`@arksync/node request queue overflow (${MAX_QUEUE_SIZE})`))
        return
      }
      this.requestQueue.push({
        request: req,
        resolve: resolve as (value: unknown) => void,
        reject,
      })
      this.dispatchNext()
    })
  }

  private dispatchSidecarEvent(event: SidecarEvent): void {
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
      default:
        break
    }
  }

  private generateHlc(): string {
    const now = new Date().toISOString()
    const counter = String(Math.floor(Math.random() * 999999)).padStart(6, '0')
    return `${now}:${counter}:${this.opts.deviceId}`
  }
}
