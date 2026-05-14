// Phase 3 cutover: SidecarClient переписан на thin wrapper над @kosmos/ark
// ArkClient. По умолчанию ходим через Kepler host (WS), при KOSMOS_KEPLER_OPTIONAL=1
// и недоступном Kepler — fallback на self-managed ark-core-rpc child.
//
// Public API (request / onEvent / reinit / releaseAndReset / shutdown / currentDbPath)
// сохранён по сигнатуре, но `reinit`/`releaseAndReset`/`shutdown` стали async
// (Promise<void> вместо void). Callsites уже либо внутри async, либо
// fire-and-forget — typecheck покажет, если где-то нужен `await`.

import fs from 'node:fs'
import path from 'node:path'
import { app } from 'electron'

import {
  ArkClient,
  ensureKeplerRunning,
  readSharedSelectedSpace,
} from '@kosmos/ark'

interface SidecarRequest {
  operation: string;
  [key: string]: unknown;
}

/** Event payloads pushed asynchronously by the ark-core-rpc sidecar. */
export interface SidecarEvent {
  event: string;
  [key: string]: unknown;
}
export type SidecarEventListener = (event: SidecarEvent) => void;

function getAppDataPath(): string {
  return process.env.KOSMOS_TEST_APPDATA
    ? path.resolve(process.env.KOSMOS_TEST_APPDATA)
    : app.getPath('appData')
}

function getSidecarBinaryPath() {
  const binaryName = process.platform === 'win32' ? 'ark-core-rpc.exe' : 'ark-core-rpc'

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'ark-core', binaryName)
  }

  // ARK is the canonical runtime for Delphi; there is no Delphi-specific DB sidecar fallback.
  const repoRoot = path.resolve(process.env.APP_ROOT ?? process.cwd(), '..', '..', '..')
  const arkCorePaths = [
    path.join(repoRoot, 'packages', 'ark-core', 'rust', 'target', 'release', binaryName),
    path.join(repoRoot, 'packages', 'ark-core', 'rust', 'target', 'debug', binaryName),
  ]
  for (const p of arkCorePaths) {
    if (fs.existsSync(p)) return p
  }

  return arkCorePaths[0]
}

/** Kepler optional by default. `KOSMOS_REQUIRE_KEPLER=1` для строгого режима. */
function isKeplerRequired(): boolean {
  return process.env.KOSMOS_REQUIRE_KEPLER === '1'
}

function isKeplerOptional(): boolean {
  return !isKeplerRequired()
}

function defaultSpaceId(): string {
  try {
    const selection = readSharedSelectedSpace(getAppDataPath())
    return selection?.spaceId ?? 'delphi-default'
  } catch {
    return 'delphi-default'
  }
}

async function registerDelphiCommands(client: ArkClient): Promise<void> {
  try {
    await client.commands.register([
      { id: 'delphi:task:create', title: 'Создать задачу', subtitle: 'Delphi', category: 'action' },
      { id: 'delphi:task:today', title: 'Открыть сегодняшние задачи', subtitle: 'Delphi', category: 'action' },
    ])
    client.commands.onInvoked((event) => {
      if (!event.id.startsWith('delphi:')) return
      console.log(`[delphi] command invoked: ${event.id}`, event.params)
    })
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.warn(`[delphi.sidecar] command registration skipped: ${msg}`)
  }
}

class SidecarClient {
  private arkClient: ArkClient | null = null
  private arkClientPromise: Promise<ArkClient> | null = null
  private dbPathOverride: string | null = null
  private eventUnsubscribe: (() => void) | null = null
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

  /** Получить ArkClient в нужном режиме (Kepler kepler-mode или self-managed fallback). */
  async getArkClient(): Promise<ArkClient> {
    if (this.arkClient) return this.arkClient
    if (this.arkClientPromise) return this.arkClientPromise

    this.arkClientPromise = (async () => {
      const spaceId = defaultSpaceId()
      const deviceId = `delphi-${process.platform}`
      const deviceName = 'Delphi'

      const state = await ensureKeplerRunning({
        appDataPath: getAppDataPath(),
        waitMs: 10000,
        autoLaunch: !isKeplerOptional(),
      })

      let client: ArkClient
      switch (state.kind) {
        case 'connected': {
          console.log(
            `[delphi.sidecar] using Kepler host (pid ${state.lock.pid}, ws_port ${state.lock.ws_port})`,
          )
          client = new ArkClient({
            spaceId,
            deviceId,
            deviceName,
            keplerLock: state.lock,
          })
          break
        }
        case 'incompatible-version': {
          throw new Error(
            `Kepler protocol mismatch: server ${state.keplerVersion.major}.${state.keplerVersion.minor}.${state.keplerVersion.patch}, ` +
              `client expects ${state.clientMajor}.x.`,
          )
        }
        case 'launch-failed':
        case 'not-installed': {
          if (isKeplerRequired()) {
            const detail =
              state.kind === 'not-installed'
                ? `checked: ${state.checkedPaths.join(', ') || '(no candidates)'}`
                : state.reason
            throw new Error(
              `Delphi запущен с KOSMOS_REQUIRE_KEPLER=1, но Kepler ${state.kind} (${detail}). ` +
                `Установи Kosmos Kepler или сними флаг.`,
            )
          }
          console.log(
            `[delphi.sidecar] Kepler ${state.kind} — standalone mode, sync disabled`,
          )
          const dbPath =
            this.dbPathOverride ?? path.join(getAppDataPath(), 'Kosmos', 'ark.db')
          fs.mkdirSync(path.dirname(dbPath), { recursive: true })
          client = new ArkClient({
            spaceId,
            deviceId,
            deviceName,
            dbPath,
            sidecarPath: getSidecarBinaryPath(),
          })
          break
        }
      }

      this.eventUnsubscribe = client.onArkEvent((event) => {
        this.dispatchEvent(event as SidecarEvent)
      })

      if (state.kind === 'connected') {
        await registerDelphiCommands(client)
      }

      this.arkClient = client
      return client
    })()

    try {
      return await this.arkClientPromise
    } catch (e) {
      this.arkClientPromise = null
      throw e
    }
  }

  async request<T>(request: SidecarRequest): Promise<T> {
    const client = await this.getArkClient()
    return client.invokeOperation<T>(request)
  }

  /** Get the current DB path override. */
  get currentDbPath(): string | null {
    return this.dbPathOverride
  }

  /**
   * Re-init под другой DB path. В kepler-mode Kepler владеет DB — `dbPath`
   * запоминается, но фактически Kepler нужно switch отдельным op'ом (Phase 5+).
   * В self-managed mode — пересоздаём ArkClient с новым dbPath.
   */
  async reinit(dbPath: string): Promise<void> {
    this.dbPathOverride = dbPath
    await this.disposeClient()
  }

  /** Kill the sidecar process and reset to default DB path. */
  async releaseAndReset(): Promise<void> {
    this.dbPathOverride = null
    await this.disposeClient()
  }

  async shutdown(): Promise<void> {
    await this.disposeClient()
    this.eventListeners.clear()
  }

  private async disposeClient(): Promise<void> {
    if (this.eventUnsubscribe) {
      this.eventUnsubscribe()
      this.eventUnsubscribe = null
    }
    const client = this.arkClient
    this.arkClient = null
    this.arkClientPromise = null
    if (client) {
      try {
        await client.stop()
      } catch {
        // ignore — best-effort cleanup
      }
    }
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
  status: number;
  scheduledDate?: string | null;
  deadline?: string | null;
  sortOrder: number;
  colorTag?: string | null;
  areaId?: string | null;
  createdAt: string;
}

interface WireProject {
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

interface WireLoadAllData {
  todos: TodoItem[];
  projects: WireProject[];
  areas: Area[];
  tags: Tag[];
  headings: Heading[];
}

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
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

// ---------------------------------------------------------------------------
// Wrapper functions
// ---------------------------------------------------------------------------

function projectStatusToWire(status: number): string {
  switch (status) {
    case 1:
      return 'someday'
    case 2:
      return 'completed'
    case 0:
    default:
      return 'active'
  }
}

function projectStatusFromWire(status: string | number | null | undefined): number {
  if (typeof status === 'number') {
    return status
  }

  switch (status) {
    case 'someday':
      return 1
    case 'completed':
      return 2
    case 'active':
    default:
      return 0
  }
}

function mapWireProject(project: WireProject): Project {
  return {
    ...project,
    status: projectStatusFromWire(project.status),
  }
}

export function dbLoadAll(): Promise<LoadAllData> {
  return sidecar.request<WireLoadAllData>({ operation: 'load_all' }).then(data => ({
    ...data,
    projects: data.projects.map(mapWireProject),
  }))
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
  const wireProject: WireProject = {
    ...project,
    status: projectStatusToWire(project.status),
  }
  return sidecar.request<void>({ operation: 'upsert_project', project: wireProject })
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

export function arkListObjects(): Promise<ArkObjectRecord[]> {
  return sidecar.request<ArkObjectRecord[]>({ operation: 'list_objects' })
}

export function arkGetObjectType(id: string): Promise<ArkObjectTypeRecord | null> {
  return sidecar.request<ArkObjectTypeRecord | null>({ operation: 'get_object_type', id })
}

export function arkUpsertObjectType(objectType: ArkObjectTypeRecord): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_object_type', object_type: objectType })
}

export function arkUpsertObject(object: ArkObjectRecord): Promise<void> {
  return sidecar.request<void>({ operation: 'upsert_object', object })
}

export function arkDeleteObject(id: string): Promise<void> {
  return sidecar.request<void>({ operation: 'delete_object', id })
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
  const dataDir = path.join(getAppDataPath(), 'Kosmos')
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
  await sidecar.reinit(newDbPath)
  // Trigger initialization by doing a lightweight operation
  await dbGetSyncKv('__init__').catch(() => {})
}
