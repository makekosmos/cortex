import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { app } from 'electron'

interface SidecarRequest {
  operation: string;
  [key: string]: unknown;
}

interface SidecarResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

interface PendingRequest<T> {
  request: SidecarRequest;
  resolve: (value: T) => void;
  reject: (error: Error) => void;
}

/** Event payloads pushed asynchronously by the ark-core-rpc sidecar. */
export interface SidecarEvent {
  event: string;
  [key: string]: unknown;
}
export type SidecarEventListener = (event: SidecarEvent) => void;

function getSidecarBinaryPath() {
  const binaryName = process.platform === 'win32' ? 'ark-core-rpc.exe' : 'ark-core-rpc'

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'ark-core', binaryName)
  }

  // Try ark-core (new unified crate) first, then legacy delphi-db
  const repoRoot = path.resolve(process.env.APP_ROOT ?? process.cwd(), '..', '..', '..')
  const arkCorePaths = [
    path.join(repoRoot, 'packages', 'ark-core', 'rust', 'target', 'release', binaryName),
    path.join(repoRoot, 'packages', 'ark-core', 'rust', 'target', 'debug', binaryName),
  ]
  for (const p of arkCorePaths) {
    if (fs.existsSync(p)) return p
  }

  // Legacy fallback
  const appRoot = process.env.APP_ROOT ?? process.cwd()
  const legacyName = process.platform === 'win32' ? 'delphi-db.exe' : 'delphi-db'
  const legacyRelease = path.join(appRoot, 'sidecar', 'target', 'release', legacyName)
  if (fs.existsSync(legacyRelease)) return legacyRelease
  return path.join(appRoot, 'sidecar', 'target', 'debug', legacyName)
}

const MAX_QUEUE_SIZE = 500

class SidecarClient {
  private child: ChildProcessWithoutNullStreams | null = null
  private stdoutChunks: Buffer[] = []
  private stdoutLength = 0
  private stderrBuffer = ''
  private requestQueue: Array<PendingRequest<unknown>> = []
  private activeRequest: PendingRequest<unknown> | null = null
  private initialized = false
  private dbPathOverride: string | null = null
  private eventListeners: Set<SidecarEventListener> = new Set()

  /** Subscribe to async events pushed by the ark-core-rpc sidecar. */
  onEvent(listener: SidecarEventListener): () => void {
    this.eventListeners.add(listener)
    return () => {
      this.eventListeners.delete(listener)
    }
  }

  private dispatchEvent(event: SidecarEvent): void {
    for (const listener of this.eventListeners) {
      try {
        listener(event)
      } catch (err) {
        console.warn('[Sidecar] event listener threw:', err)
      }
    }
  }

  private ensureChild() {
    if (this.child) {
      return this.child
    }

    const binaryPath = getSidecarBinaryPath()
    const child = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    })

    child.stderr.setEncoding('utf8')

    child.stdout.on('data', (chunk: Buffer | string) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
      this.stdoutChunks.push(buf)
      this.stdoutLength += buf.length
      this.flushStdout()
    })

    child.stderr.on('data', (chunk: string) => {
      this.stderrBuffer += chunk
    })

    child.on('error', error => {
      this.failAll(error instanceof Error ? error : new Error(String(error)))
    })

    child.on('close', code => {
      const reason = this.stderrBuffer.trim() || `delphi-db exited with ${code}`
      this.failAll(new Error(reason))
    })

    this.child = child
    this.stdoutChunks = []
    this.stdoutLength = 0
    this.stderrBuffer = ''

    if (!this.initialized) {
      this.initialized = true
      const dbPath = this.dbPathOverride ?? path.join(app.getPath('appData'), 'Kepler', 'ark.db')
      // Ensure parent directory exists (for per-space paths)
      fs.mkdirSync(path.dirname(dbPath), { recursive: true })
      const initMsg = JSON.stringify({ operation: 'init', dbPath })
      child.stdin.write(`${initMsg}\n`)
    }

    return child
  }

  private flushStdout() {
    // Merge chunks into a single buffer to find newlines
    const merged = Buffer.concat(this.stdoutChunks, this.stdoutLength)
    this.stdoutChunks = []
    this.stdoutLength = 0

    let searchFrom = 0
    while (true) {
      const newlineIndex = merged.indexOf(0x0a, searchFrom) // '\n'
      if (newlineIndex === -1) {
        // Put remaining data back as a single chunk
        if (searchFrom < merged.length) {
          const remaining = merged.subarray(searchFrom)
          this.stdoutChunks.push(remaining)
          this.stdoutLength = remaining.length
        }
        return
      }

      const rawLine = merged.subarray(searchFrom, newlineIndex).toString('utf8').trim()
      searchFrom = newlineIndex + 1

      if (!rawLine) {
        continue
      }

      let parsed: unknown
      try {
        parsed = JSON.parse(rawLine)
      } catch (error) {
        // If we're mid-request, fail it; otherwise drop the garbage line.
        const pending = this.activeRequest
        this.activeRequest = null
        if (pending) {
          pending.reject(error instanceof Error ? error : new Error(String(error)))
          this.dispatchNext()
        }
        continue
      }

      // Event frames are distinguished by having an `event` field (and no `ok`
      // field). They flow on the same stdout stream as responses but must not
      // consume the activeRequest slot.
      if (parsed && typeof parsed === 'object' && 'event' in (parsed as Record<string, unknown>) && !('ok' in (parsed as Record<string, unknown>))) {
        this.dispatchEvent(parsed as SidecarEvent)
        continue
      }

      const pending = this.activeRequest
      this.activeRequest = null

      if (!pending) {
        // Spurious response — no caller is waiting. Drop it quietly to
        // avoid wedging the queue.
        continue
      }

      const response = parsed as SidecarResponse<unknown>
      if (!response.ok) {
        pending.reject(new Error(response.error || 'ark-core-rpc request failed'))
      } else {
        pending.resolve(response.data)
      }

      this.dispatchNext()
    }
  }

  private dispatchNext() {
    if (this.activeRequest || this.requestQueue.length === 0) {
      return
    }

    const nextRequest = this.requestQueue.shift()
    if (!nextRequest) {
      return
    }

    const child = this.ensureChild()
    this.activeRequest = nextRequest

    try {
      child.stdin.write(`${JSON.stringify(nextRequest.request)}\n`)
    } catch (error) {
      this.activeRequest = null
      nextRequest.reject(error instanceof Error ? error : new Error(String(error)))
      this.resetChild()
      this.dispatchNext()
    }
  }

  private failAll(error: Error) {
    const pending = this.activeRequest
    const queued = this.requestQueue.splice(0)

    this.activeRequest = null
    this.resetChild()

    if (pending) {
      pending.reject(error)
    }

    queued.forEach(request => request.reject(error))
  }

  private resetChild() {
    if (this.child) {
      this.child.stdout.removeAllListeners()
      this.child.stderr.removeAllListeners()
      this.child.removeAllListeners()

      if (!this.child.killed) {
        this.child.kill()
      }
    }

    this.child = null
    this.stdoutChunks = []
    this.stdoutLength = 0
    this.stderrBuffer = ''
    this.initialized = false
  }

  request<T>(request: SidecarRequest): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (this.requestQueue.length >= MAX_QUEUE_SIZE) {
        reject(new Error(`delphi-db request queue overflow (${MAX_QUEUE_SIZE})`))
        return
      }
      this.requestQueue.push({
        request,
        resolve: resolve as (value: unknown) => void,
        reject,
      })
      this.dispatchNext()
    })
  }

  /** Get the current DB path (for checking if sidecar holds a specific file). */
  get currentDbPath(): string | null {
    return this.dbPathOverride
  }

  /** Reinitialize the sidecar with a different DB path (used for space switching). */
  reinit(dbPath: string): void {
    this.dbPathOverride = dbPath
    this.resetChild()
  }

  /** Kill the sidecar process and reset to default DB path. */
  releaseAndReset(): void {
    this.dbPathOverride = null
    this.resetChild()
  }

  shutdown() {
    this.requestQueue.splice(0).forEach(request => {
      request.reject(new Error('ark-core-rpc client shut down'))
    })

    if (this.activeRequest) {
      this.activeRequest.reject(new Error('ark-core-rpc client shut down'))
      this.activeRequest = null
    }

    if (this.child && !this.child.killed) {
      this.child.kill()
    }

    this.resetChild()
    this.eventListeners.clear()
  }
}

export const sidecar = new SidecarClient()

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RecurrenceData {
  frequency: number;
  interval: number;
  recurrenceType: number;
  daysOfWeek?: number[] | null;
  endDate?: string | null;
}

export interface ChecklistItem {
  id: string;
  title: string;
  isCompleted: boolean;
  sortOrder: number;
  todoItemId?: string | null;
}

export interface TodoItem {
  id: string;
  title: string;
  notes?: string | null;
  priority: number;
  scheduledDate?: string | null;
  deadline?: string | null;
  reminderDate?: string | null;
  isToday: boolean;
  isEvening: boolean;
  isSomeday: boolean;
  isCompleted: boolean;
  completedAt?: string | null;
  isCancelled: boolean;
  cancelledAt?: string | null;
  isTrashed: boolean;
  sortOrder: number;
  createdAt: string;
  headingId?: string | null;
  projectId?: string | null;
  areaId?: string | null;
  tagIds: string[];
  checklistItems: ChecklistItem[];
  recurrenceRule?: RecurrenceData | null;
}

export interface Project {
  id: string;
  title: string;
  notes?: string | null;
  status: string;
  scheduledDate?: string | null;
  deadline?: string | null;
  sortOrder: number;
  colorTag?: string | null;
  areaId?: string | null;
  createdAt: string;
}

export interface Area {
  id: string;
  title: string;
  sortOrder: number;
  createdAt: string;
}

export interface Tag {
  id: string;
  title: string;
  color?: string | null;
  createdAt: string;
}

export interface Heading {
  id: string;
  title: string;
  sortOrder: number;
  projectId: string;
}

export interface LoadAllData {
  todos: TodoItem[];
  projects: Project[];
  areas: Area[];
  tags: Tag[];
  headings: Heading[];
}

// ---------------------------------------------------------------------------
// Wrapper functions
// ---------------------------------------------------------------------------

export function dbLoadAll(): Promise<LoadAllData> {
  return sidecar.request<LoadAllData>({ operation: 'load_all' })
}

export function dbUpsertTodo(todo: TodoItem): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_todo', todo })
}

export function dbDeleteTodo(id: string): Promise<void> {
  return sidecar.request<void>({ operation: 'delete_todo', id })
}

export function dbBatchUpsertTodos(todos: TodoItem[]): Promise<void> {
  return sidecar.request<void>({ operation: 'batch_upsert_todos', todos })
}

export function dbUpsertProject(project: Project): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_project', project })
}

export function dbDeleteProject(id: string): Promise<void> {
  return sidecar.request<void>({ operation: 'delete_project', id })
}

export function dbUpsertArea(area: Area): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_area', area })
}

export function dbUpsertTag(tag: Tag): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_tag', tag })
}

export function dbUpsertHeading(heading: Heading): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_heading', heading })
}

export function dbDeleteHeading(id: string): Promise<void> {
  return sidecar.request<void>({ operation: 'delete_heading', id })
}

export function dbGetSyncKv(key: string): Promise<string | null> {
  return sidecar.request<string | null>({ operation: 'get_sync_kv', key })
}

export function dbSetSyncKv(key: string, value: string): Promise<void> {
  return sidecar.request<void>({ operation: 'set_sync_kv', key, value })
}

export async function dbClearAll(): Promise<void> {
  await sidecar.request({ operation: 'clear_all' })
}

export async function dbDeleteTrashed(): Promise<number> {
  return sidecar.request<number>({ operation: 'delete_trashed' })
}

// ---------------------------------------------------------------------------
// Sync wrapper functions (new in ark-core-rpc runtime)
// ---------------------------------------------------------------------------

/** Minimal sync entity shape matching the Rust SyncEntity on the wire. */
export interface SyncEntityPayload {
  type: string;
  id: string;
  data: Record<string, unknown>;
  hlc: string;
  deleted?: boolean;
}

export interface ConnectedPeer {
  device_id: string;
  device_name: string;
}

export interface StartSyncParams {
  spaceId: string;
  deviceId: string;
  deviceName?: string;
  port?: number;
  seedAddresses?: string[];
}

export function syncStart(params: StartSyncParams): Promise<boolean> {
  return sidecar.request<boolean>({
    operation: 'start_sync',
    space_id: params.spaceId,
    device_id: params.deviceId,
    device_name: params.deviceName ?? null,
    port: params.port ?? null,
    seed_addresses: params.seedAddresses ?? null,
  })
}

export function syncStop(): Promise<boolean> {
  return sidecar.request<boolean>({ operation: 'stop_sync' })
}

export function syncLeaveSpace(): Promise<boolean> {
  return sidecar.request<boolean>({ operation: 'leave_space' })
}

export function syncBroadcastChange(entity: SyncEntityPayload): Promise<boolean> {
  return sidecar.request<boolean>({ operation: 'broadcast_change', entity })
}

export function syncGetConnectedPeers(): Promise<ConnectedPeer[]> {
  return sidecar.request<ConnectedPeer[]>({ operation: 'get_connected_peers' })
}

export function syncAddSeedPeer(addresses: string[]): Promise<boolean> {
  return sidecar.request<boolean>({ operation: 'add_seed_peer', addresses })
}

export function syncGetOwnAddresses(port?: number): Promise<string[]> {
  return sidecar.request<string[]>({ operation: 'get_own_addresses', port: port ?? null })
}

export function syncGetHostDeviceName(): Promise<string> {
  return sidecar.request<string>({ operation: 'get_host_device_name' })
}

/**
 * Switch the sidecar to a per-space DB.
 * The old sidecar process is killed and a new one is started with the space DB path.
 */
export async function dbSwitchSpace(spaceId: string): Promise<void> {
  const dataDir = path.join(app.getPath('appData'), 'Kepler')
  const spaceDir = path.join(dataDir, 'spaces', spaceId)
  fs.mkdirSync(spaceDir, { recursive: true })
  const newDbPath = path.join(spaceDir, 'ark.db')
  // Auto-migrate legacy delphi.db → ark.db
  const legacyPath = path.join(spaceDir, 'delphi.db')
  if (!fs.existsSync(newDbPath) && fs.existsSync(legacyPath)) {
    fs.renameSync(legacyPath, newDbPath)
    // Also rename WAL/SHM if present
    for (const suffix of ['-wal', '-shm']) {
      const src = legacyPath + suffix
      if (fs.existsSync(src)) fs.renameSync(src, newDbPath + suffix)
    }
    console.log(`[Sidecar] Migrated delphi.db → ark.db for space ${spaceId}`)
  }
  sidecar.reinit(newDbPath)
  // Trigger initialization by doing a lightweight operation
  await dbGetSyncKv('__init__').catch(() => {})
}
