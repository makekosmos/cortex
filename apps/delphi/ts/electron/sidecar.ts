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

function getSidecarBinaryPath() {
  const appRoot = process.env.APP_ROOT ?? process.cwd()
  const binaryName = process.platform === 'win32' ? 'delphi-db.exe' : 'delphi-db'

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'delphi-db', binaryName)
  }

  const releasePath = path.join(appRoot, 'sidecar', 'target', 'release', binaryName)
  if (fs.existsSync(releasePath)) {
    return releasePath
  }

  return path.join(appRoot, 'sidecar', 'target', 'debug', binaryName)
}

class SidecarClient {
  private child: ChildProcessWithoutNullStreams | null = null
  private stdoutBuffer = ''
  private stderrBuffer = ''
  private requestQueue: Array<PendingRequest<unknown>> = []
  private activeRequest: PendingRequest<unknown> | null = null
  private initialized = false
  private dbPathOverride: string | null = null

  private ensureChild() {
    if (this.child) {
      return this.child
    }

    const binaryPath = getSidecarBinaryPath()
    const child = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    })

    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')

    child.stdout.on('data', (chunk: string) => {
      this.stdoutBuffer += chunk
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
    this.stdoutBuffer = ''
    this.stderrBuffer = ''

    if (!this.initialized) {
      this.initialized = true
      const dbPath = this.dbPathOverride ?? path.join(app.getPath('userData'), 'delphi.db')
      // Ensure parent directory exists (for per-space paths)
      fs.mkdirSync(path.dirname(dbPath), { recursive: true })
      const initMsg = JSON.stringify({ operation: 'init', dbPath })
      child.stdin.write(`${initMsg}\n`)
    }

    return child
  }

  private flushStdout() {
    while (true) {
      const newlineIndex = this.stdoutBuffer.indexOf('\n')
      if (newlineIndex === -1) {
        return
      }

      const rawLine = this.stdoutBuffer.slice(0, newlineIndex).trim()
      this.stdoutBuffer = this.stdoutBuffer.slice(newlineIndex + 1)

      if (!rawLine) {
        continue
      }

      const pending = this.activeRequest
      this.activeRequest = null

      if (!pending) {
        continue
      }

      try {
        const response = JSON.parse(rawLine) as SidecarResponse<unknown>
        if (!response.ok) {
          pending.reject(new Error(response.error || 'delphi-db request failed'))
        } else {
          pending.resolve(response.data)
        }
      } catch (error) {
        pending.reject(error instanceof Error ? error : new Error(String(error)))
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
      this.child.removeAllListeners()
      this.child.stdout.removeAllListeners()
      this.child.stderr.removeAllListeners()

      if (!this.child.killed) {
        this.child.kill()
      }
    }

    this.child = null
    this.stdoutBuffer = ''
    this.stderrBuffer = ''
    this.initialized = false
  }

  request<T>(request: SidecarRequest): Promise<T> {
    return new Promise<T>((resolve, reject) => {
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
      request.reject(new Error('delphi-db client shut down'))
    })

    if (this.activeRequest) {
      this.activeRequest.reject(new Error('delphi-db client shut down'))
      this.activeRequest = null
    }

    if (this.child && !this.child.killed) {
      this.child.kill()
    }

    this.resetChild()
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

/**
 * Switch the sidecar to a per-space DB.
 * The old sidecar process is killed and a new one is started with the space DB path.
 */
export async function dbSwitchSpace(spaceId: string): Promise<void> {
  const newDbPath = path.join(app.getPath('userData'), 'spaces', spaceId, 'delphi.db')
  sidecar.reinit(newDbPath)
  // Trigger initialization by doing a lightweight operation
  await dbGetSyncKv('__init__').catch(() => {})
}
