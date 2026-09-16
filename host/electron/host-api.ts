import {
  ArkClient,
  ensureEngineRunning,
  ReconnectingEngineClient,
  type EngineLockInfo,
} from "@makekosmos/ark";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { SAFE_ID } from "./app-navigation";

export type AppLaunch = {
  id: string;
  version: string;
  name: string;
  launch_url: string;
  permissions: PermissionGrant[];
  launch_id: string;
  ttl_seconds: number;
  expires_at: string;
  /** Runtime-issued authority; never cross the preload/renderer boundary. */
  broker_token?: string;
  /** Launch-scoped Engine endpoint; retained only by the main-process manifest. */
  data_api?: string;
  /** Runtime marker selecting the launch-scoped v2 broker contract. */
  manifest_schema_version?: number;
  /** Concrete ARK type ids allowed for v2 read/subscribe event fan-out. */
  effective_read_types?: string[];
  /** Runtime-issued non-entity events allowed for v2 fan-out. */
  effective_events?: string[];
};
export type LaunchRenewal = {
  launch_id: string;
  ttl_seconds: number;
  expires_at: string;
};
export type AppResolve = {
  id: string;
  version: string;
  name: string;
  permissions: PermissionGrant[];
  enabled: true;
  revoked: false;
};
export type DirectoryGrant = Readonly<{
  persistentGrantId: string;
  label: string;
}>;
export type PermissionGrant = { capability: string; scopes?: string[] };
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
export type JsonRecord = { [key: string]: JsonValue };
export type SidecarEvent = JsonRecord & { event: string };
export type CrashDetails = { reason?: JsonValue; exitCode?: JsonValue };
type CrashMetadata = {
  component: string;
  app_id?: string;
  version?: string;
  reason: string;
  exit_code: number | null;
};

export type EngineResult<T> = { ok: true; data: T } | { ok: false; message: string };
/**
 * `/v1/user-data` result preserving the Engine's raw error code so callers
 * can distinguish typed failures (not-found/invalid-key/too-large/
 * unknown-root) from transport errors.
 */
export type UserDataCallResult<T> = { ok: true; data: T } | { ok: false; error: string };

const API_VERSION = "1.0.0";
const SAFE_LAUNCH_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
export const isJsonString = (value: JsonValue | undefined): value is string =>
  typeof value === "string";
export const isJsonNumber = (value: JsonValue | undefined): value is number =>
  typeof value === "number";
export const isJsonRecord = (value: JsonValue | undefined): value is JsonRecord =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);
const isString = isJsonString;
const isNumber = isJsonNumber;
const isRecord = isJsonRecord;

const READ_OPS = new Set([
  "list_objects",
  "get_object",
  "search_objects",
  "list_object_types",
  "get_object_type",
  "list_object_links",
  "get_sync_kv",
]);
const WRITE_OPS = new Set([
  "upsert_object",
  "delete_object",
  "upsert_object_type",
  "delete_object_type",
  "upsert_object_link",
  "delete_object_link",
  "set_sync_kv",
]);

export function requiredCapability(operation: string): "ark.read" | "ark.write" | undefined {
  return READ_OPS.has(operation) ? "ark.read" : WRITE_OPS.has(operation) ? "ark.write" : undefined;
}

export function hasManifestGrant(
  permissions: PermissionGrant[] | undefined,
  capability: "ark.read" | "ark.write",
  operation: string,
): boolean {
  return (
    permissions?.some(
      (grant) =>
        grant.capability === capability &&
        Array.isArray(grant.scopes) &&
        grant.scopes.length > 0 &&
        grant.scopes.includes(operation),
    ) ?? false
  );
}

export function hasArkGrant(permissions: PermissionGrant[] | undefined): boolean {
  return (
    permissions?.some(
      (grant) =>
        (grant.capability === "ark.read" || grant.capability === "ark.write") &&
        Array.isArray(grant.scopes) &&
        grant.scopes.length > 0,
    ) ?? false
  );
}

/**
 * A launch is v2 only when Engine explicitly marks it (or supplies its
 * launch authority). Missing v2 authority must never fall back to global ARK.
 */
export function isV2Launch(
  launch: Pick<AppLaunch, "manifest_schema_version" | "broker_token" | "data_api">,
): boolean {
  return (
    launch.manifest_schema_version === 2 ||
    launch.broker_token !== undefined ||
    launch.data_api !== undefined
  );
}

export function hasLaunchReadPermission(launch: AppLaunch, event?: JsonRecord): boolean {
  if (isV2Launch(launch)) {
    const readTypes = Array.isArray(launch.effective_read_types) ? launch.effective_read_types : [];
    const events = Array.isArray(launch.effective_events) ? launch.effective_events : [];
    if (!event) return readTypes.length > 0 || events.length > 0;
    if (event.event === "agents_event") return events.includes("agents_event");
    const typeId = eventTypeId(event);
    return typeId !== undefined && readTypes.includes(typeId);
  }
  return hasArkReadPermission(launch.permissions);
}

function eventTypeId(event: JsonRecord): string | undefined {
  const readTypeId = (value: JsonValue | undefined): string | undefined => {
    if (!isRecord(value)) return undefined;
    const record = value;
    return isString(record.type_id)
      ? record.type_id
      : isString(record.typeId)
        ? record.typeId
        : undefined;
  };

  const direct = readTypeId(event);
  if (direct) return direct;
  const entity = event.entity;
  const entityType = readTypeId(entity);
  if (entityType) return entityType;
  if (isRecord(entity)) {
    const data = entity.data;
    const dataType = readTypeId(data);
    if (dataType) return dataType;
  }
  return readTypeId(event.data);
}

export function launchRenewalDelayMs(
  expiresAt: string,
  now = Date.now(),
  retry = false,
): number | null {
  const expiry = Date.parse(expiresAt);
  if (!Number.isFinite(expiry)) return null;
  const untilExpiry = expiry - now;
  if (untilExpiry <= 1_000) return null;
  return retry
    ? Math.max(1_000, Math.min(30_000, untilExpiry - 1_000))
    : Math.max(1_000, untilExpiry - 30_000);
}

function hasArkReadPermission(permissions: PermissionGrant[] | undefined): boolean {
  return (
    permissions?.some(
      (grant) =>
        grant.capability === "ark.read" && Array.isArray(grant.scopes) && grant.scopes.length > 0,
    ) ?? false
  );
}

export function hasLauncherGrant(
  permissions: PermissionGrant[] | undefined,
  operation: string,
): boolean {
  return (
    permissions?.some(
      (grant) =>
        grant.capability === "launcher.search" &&
        Array.isArray(grant.scopes) &&
        grant.scopes.includes(operation),
    ) ?? false
  );
}

export function isInstalledEnabledApp<
  T extends {
    id?: unknown;
    enabled?: unknown;
    revoked?: unknown;
  },
>(item: T): item is T & { id: string; enabled: true; revoked: false } {
  return (
    SAFE_ID.test(typeof item.id === "string" ? item.id : "") &&
    item.enabled === true &&
    item.revoked === false
  );
}

const CRASH_REASONS = new Set([
  "clean-exit",
  "abnormal-exit",
  "killed",
  "crashed",
  "oom",
  "launch-failed",
  "integrity-failure",
]);

export function redactedCrashMetadata(
  appId: string | undefined,
  version: string | undefined,
  details: CrashDetails,
) {
  const metadata: CrashMetadata = {
    component: "renderer",
    reason:
      isString(details.reason) && CRASH_REASONS.has(details.reason) ? details.reason : "unknown",
    exit_code:
      isNumber(details.exitCode) && Number.isInteger(details.exitCode) ? details.exitCode : null,
  };
  if (appId) metadata.app_id = appId.slice(0, 128);
  if (version) metadata.version = version.slice(0, 64);
  return metadata;
}

export class EngineClient {
  private lock: EngineLockInfo | null = null;
  private readonly reconnectingArk: ReconnectingEngineClient<ArkClient>;

  constructor(
    private readonly appDataPath: string,
    private readonly dataDir?: string,
  ) {
    this.reconnectingArk = new ReconnectingEngineClient({
      discover: () =>
        ensureEngineRunning({
          appDataPath: this.appDataPath,
          dataDir: this.dataDir,
          clientProtocolMajor: 1,
          autoLaunch: true,
        }),
      createTransport: (lock) => {
        this.lock = lock;
        return new ArkClient({
          spaceId: "desktop-host",
          deviceId: `desktop-host-${process.pid}`,
          engineLock: lock,
          engineClientClass: "desktop-host",
          engineClientVersion: "1.0.0",
        });
      },
    });
  }

  async ensureEngineRunning(): Promise<EngineResult<void>> {
    try {
      await this.reconnectingArk.getTransport();
      return { ok: true, data: undefined };
    } catch {
      return {
        ok: false,
        message: "Engine API v1 недоступен.",
      };
    }
  }

  onArkEvent(listener: (event: SidecarEvent) => void): () => void {
    // SAFETY: Ark event payloads are emitted by the typed reconnecting client.
    const unsubscribe = this.reconnectingArk.onArkEvent((event) => listener(event as SidecarEvent));
    void this.ensureArkClient();
    return unsubscribe;
  }

  private async ensureArkClient(): Promise<ArkClient | null> {
    try {
      return await this.reconnectingArk.getTransport();
    } catch {
      return null;
    }
  }

  private async request<T>(
    path: string,
    init: RequestInit = {},
    replay = false,
  ): Promise<EngineResult<T>> {
    try {
      return await this.reconnectingArk.execute(() => this.requestWithCurrentLock<T>(path, init), {
        idempotent: replay,
      });
    } catch {
      return {
        ok: false,
        message: "Не удалось связаться с Engine. Повторите попытку.",
      };
    }
  }

  private async requestWithCurrentLock<T>(
    path: string,
    init: RequestInit = {},
  ): Promise<EngineResult<T>> {
    if (!this.lock) {
      const connected = await this.ensureEngineRunning();
      if (!connected.ok) return connected;
    }
    const lock = this.lock;
    if (!lock) return { ok: false, message: "Engine API v1 недоступен." };
    const response = await fetch(`http://127.0.0.1:${lock.http_port}${path}`, {
      ...init,
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "Content-Type": "application/json",
        "X-Kosmos-Api-Version": API_VERSION,
        "X-Kosmos-Client-Class": "desktop-host",
        "X-Kosmos-Client-Version": "1.0.0",
        "X-Kosmos-Client-Pid": String(process.pid),
        ...init.headers,
      },
      signal: AbortSignal.timeout(12_000),
    });
    if ([401, 426].includes(response.status))
      throw Object.assign(new Error(`HTTP ${response.status}`), {
        status: response.status,
      });
    // SAFETY: Engine responses are checked below before their generic payload is returned.
    const value = (await response.json()) as { ok?: boolean; data?: T; error?: string } | null;
    if (response.status === 403 && value?.error !== "forbidden")
      throw Object.assign(new Error("HTTP 403"), { status: 403 });
    if (!response.ok || !value || value.ok !== true || !("data" in value)) {
      const code = value?.error?.match(
        /(?:^|: )(forbidden|invalid-request|not-found|conflict|timeout|unavailable)$/,
      )?.[1];
      return {
        ok: false,
        message: code ? `Engine отклонил операцию: ${code}.` : "Engine отклонил операцию.",
      };
    }
    // SAFETY: the property check above rejects a missing data key; JSON cannot encode undefined.
    return { ok: true, data: value.data as T };
  }

  async resolveApp(id: string, version?: string): Promise<EngineResult<AppResolve>> {
    if (!SAFE_ID.test(id)) return { ok: false, message: "Некорректный идентификатор приложения." };
    return this.request<AppResolve>(
      "/v1/apps/resolve",
      {
        method: "POST",
        body: JSON.stringify(version ? { id, version } : { id }),
      },
      true,
    );
  }

  async launchApp(id: string, version?: string): Promise<EngineResult<AppLaunch>> {
    if (!SAFE_ID.test(id)) return { ok: false, message: "Некорректный идентификатор приложения." };
    return this.request<AppLaunch>("/v1/apps/launch", {
      method: "POST",
      body: JSON.stringify(version ? { id, version } : { id }),
    });
  }

  async revokeApp(launchId: string): Promise<EngineResult<{ launch_id: string; revoked: true }>> {
    if (!SAFE_LAUNCH_ID.test(launchId))
      return { ok: false, message: "Некорректный идентификатор запуска." };
    return this.request<{ launch_id: string; revoked: true }>(`/v1/apps/launch/${launchId}`, {
      method: "DELETE",
    });
  }

  async renewLaunch(launchId: string, brokerToken: string): Promise<EngineResult<LaunchRenewal>> {
    if (!SAFE_LAUNCH_ID.test(launchId) || !isSafeBrokerToken(brokerToken)) {
      return { ok: false, message: "Некорректное продление запуска." };
    }
    return this.request<LaunchRenewal>(`/v1/apps/launch/${launchId}/renew`, {
      method: "POST",
      headers: { "X-Kosmos-Launch-Token": brokerToken },
    });
  }

  async registerDirectoryGrant(
    launchId: string,
    brokerToken: string,
    absoluteRoot: string,
  ): Promise<EngineResult<DirectoryGrant>> {
    if (
      !SAFE_LAUNCH_ID.test(launchId) ||
      !isSafeBrokerToken(brokerToken) ||
      !isAbsolutePath(absoluteRoot)
    ) {
      return { ok: false, message: "Некорректный выбранный каталог." };
    }
    const result = await this.request<JsonRecord>(`/v1/apps/launch/${launchId}/grants/directory`, {
      method: "POST",
      headers: { "X-Kosmos-Launch-Token": brokerToken },
      body: JSON.stringify({ selected_root: absoluteRoot }),
    });
    if (!result.ok) return result;
    const persistentGrantId = result.data.persistentGrantId;
    const label = result.data.label;
    if (!isSafeText(persistentGrantId) || !isSafeText(label)) {
      return { ok: false, message: "Engine вернул некорректный grant каталога." };
    }
    return { ok: true, data: { persistentGrantId, label } };
  }

  /**
   * Pin the Host userData base directory to an Engine-native directory
   * handle. Every returned root id is opaque and stays valid until Engine
   * restarts, after which callers must open again.
   */
  async userDataOpenRoot(root: string): Promise<UserDataCallResult<{ rootId: string }>> {
    if (!isAbsolutePath(root)) return { ok: false, error: "invalid-request" };
    const result = await this.userDataOperation<JsonRecord>({ operation: "open_root", root }, true);
    if (!result.ok) return result;
    const rootId = result.data.root_id;
    return isString(rootId) && /^[\w-]{1,128}$/.test(rootId)
      ? { ok: true, data: { rootId } }
      : { ok: false, error: "io-error" };
  }

  async userDataRead(
    rootId: string,
    appId: string,
    key: string,
  ): Promise<UserDataCallResult<Uint8Array>> {
    const result = await this.userDataOperation<JsonRecord>(
      { operation: "read", root_id: rootId, app_id: appId, key },
      true,
    );
    if (!result.ok) return result;
    const encoded = result.data.bytes;
    // Base64 of a 25 MiB payload is ~35 MB; larger responses are malformed.
    if (!isString(encoded) || encoded.length > 36_000_000) return { ok: false, error: "io-error" };
    return { ok: true, data: new Uint8Array(Buffer.from(encoded, "base64")) };
  }

  async userDataWrite(
    rootId: string,
    appId: string,
    key: string,
    bytes: Uint8Array,
  ): Promise<UserDataCallResult<{ sizeBytes: number }>> {
    const result = await this.userDataRequest<JsonRecord>(
      {
        method: "PUT",
        headers: {
          "X-Kosmos-User-Data-Root": rootId,
          "X-Kosmos-User-Data-App": appId,
          "X-Kosmos-User-Data-Key": key,
        },
        body: bytes.slice().buffer,
      },
      true,
    );
    if (!result.ok) return result;
    const sizeBytes = result.data.size_bytes;
    return isNumber(sizeBytes) && Number.isInteger(sizeBytes) && sizeBytes >= 0
      ? { ok: true, data: { sizeBytes } }
      : { ok: false, error: "io-error" };
  }

  async userDataStat(
    rootId: string,
    appId: string,
    key: string,
  ): Promise<UserDataCallResult<{ sizeBytes: number }>> {
    const result = await this.userDataOperation<JsonRecord>(
      { operation: "stat", root_id: rootId, app_id: appId, key },
      true,
    );
    if (!result.ok) return result;
    const sizeBytes = result.data.size_bytes;
    return isNumber(sizeBytes) && Number.isInteger(sizeBytes) && sizeBytes >= 0
      ? { ok: true, data: { sizeBytes } }
      : { ok: false, error: "io-error" };
  }

  async userDataDelete(
    rootId: string,
    appId: string,
    key: string,
  ): Promise<UserDataCallResult<{ deleted: true }>> {
    const result = await this.userDataOperation<JsonRecord>(
      { operation: "delete", root_id: rootId, app_id: appId, key },
      false,
    );
    if (!result.ok) return result;
    return result.data.deleted === true
      ? { ok: true, data: { deleted: true } }
      : { ok: false, error: "io-error" };
  }

  private userDataOperation<T>(
    body: JsonRecord,
    idempotent: boolean,
  ): Promise<UserDataCallResult<T>> {
    return this.userDataRequest<T>(
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      },
      idempotent,
    );
  }

  /**
   * `/v1/user-data` shares the Engine auth handshake but keeps the raw error
   * code — the userData contract distinguishes `not-found`/`invalid-key`/
   * `too-large`/`unknown-root` from generic transport failures.
   */
  private async userDataRequest<T>(
    init: RequestInit,
    idempotent: boolean,
  ): Promise<UserDataCallResult<T>> {
    try {
      return await this.reconnectingArk.execute(() => this.userDataFetch<T>(init), {
        idempotent,
      });
    } catch {
      return { ok: false, error: "io-error" };
    }
  }

  private async userDataFetch<T>(init: RequestInit): Promise<UserDataCallResult<T>> {
    if (!this.lock) {
      const connected = await this.ensureEngineRunning();
      if (!connected.ok) return { ok: false, error: "io-error" };
    }
    const lock = this.lock;
    if (!lock) return { ok: false, error: "io-error" };
    const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/user-data`, {
      ...init,
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "X-Kosmos-Api-Version": API_VERSION,
        "X-Kosmos-Client-Class": "desktop-host",
        "X-Kosmos-Client-Version": "1.0.0",
        "X-Kosmos-Client-Pid": String(process.pid),
        ...init.headers,
      },
      signal: AbortSignal.timeout(60_000),
    });
    if ([401, 426].includes(response.status))
      throw Object.assign(new Error(`HTTP ${response.status}`), {
        status: response.status,
      });
    // SAFETY: the envelope is parsed defensively; any malformed shape maps to io-error.
    const value = (await response.json().catch(() => null)) as {
      ok?: boolean;
      data?: T;
      error?: string;
    } | null;
    if (!response.ok || !value || value.ok !== true || !("data" in value)) {
      const code = value?.error;
      return { ok: false, error: isString(code) ? code : "io-error" };
    }
    // SAFETY: the property check above rejects a missing data key; JSON cannot encode undefined.
    return { ok: true, data: value.data as T };
  }

  async getWarmTimeout(): Promise<EngineResult<0 | 300>> {
    const result = await this.request<{
      desktop_host?: { warm_timeout_seconds?: number };
    }>(
      "/v1/rpc",
      {
        method: "POST",
        body: JSON.stringify({
          operation: "engine.settings.get",
          _req_id: randomUUID(),
        }),
      },
      true,
    );
    if (!result.ok) return result;
    const timeout = result.data.desktop_host?.warm_timeout_seconds;
    return timeout === 0 || timeout === 300
      ? { ok: true, data: timeout }
      : {
          ok: false,
          message: "Настройка времени ожидания Engine некорректна.",
        };
  }

  async arkRequest(operation: string, params: JsonRecord): Promise<EngineResult<JsonValue>> {
    try {
      const data = await this.reconnectingArk.invokeOperation(
        { ...params, operation },
        { idempotent: READ_OPS.has(operation) },
      );
      // SAFETY: ArkClient returns JSON-compatible operation payloads.
      return { ok: true, data: data as JsonValue };
    } catch {
      return { ok: false, message: "Engine отклонил операцию." };
    }
  }

  async launchArkRequest(
    launchId: string,
    brokerToken: string,
    operation: string,
    params: JsonRecord,
  ): Promise<EngineResult<JsonValue>> {
    if (!SAFE_LAUNCH_ID.test(launchId) || !operation || !isSafeBrokerToken(brokerToken)) {
      return { ok: false, message: "Некорректный launch-scoped запрос ARK." };
    }
    return this.request<JsonValue>(`/v1/apps/launch/${launchId}/ark`, {
      method: "POST",
      headers: { "X-Kosmos-Launch-Token": brokerToken },
      body: JSON.stringify({ operation, params }),
    });
  }

  async arkDataRequest(request: JsonRecord): Promise<EngineResult<JsonValue>> {
    if (
      !isString(request.kind) ||
      request.kind === "raw" ||
      "operation" in request ||
      "params" in request
    )
      return { ok: false, message: "Некорректный типизированный запрос ARK." };
    try {
      const data = await this.reconnectingArk.invokeOperation(
        // SAFETY: the checks above reject raw requests and require a string kind.
        request as JsonRecord & { operation: string },
        {
          idempotent: request.kind === "read_object" || request.kind === "list_objects",
        },
      );
      // SAFETY: ArkClient returns JSON-compatible operation payloads.
      return { ok: true, data: data as JsonValue };
    } catch {
      return { ok: false, message: "Engine отклонил типизированный запрос ARK." };
    }
  }
  async launcherRequest(operation: string, params: JsonRecord): Promise<EngineResult<JsonValue>> {
    return this.request<JsonValue>("/v1/rpc", {
      method: "POST",
      body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
    });
  }
}

export { SAFE_ID };

function isSafeBrokerToken(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 256 &&
    ![...value].some((character) => {
      const code = character.charCodeAt(0);
      return code <= 0x20 || code === 0x7f;
    })
  );
}

function isAbsolutePath(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 32_768 &&
    ![...value].some(
      (character) => character.charCodeAt(0) <= 0x1f || character.charCodeAt(0) === 0x7f,
    ) &&
    (path.isAbsolute(value) || /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\"))
  );
}

function isSafeText(value: JsonValue | undefined): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 512 &&
    ![...value].some(
      (character) => character.charCodeAt(0) <= 0x1f || character.charCodeAt(0) === 0x7f,
    )
  );
}
