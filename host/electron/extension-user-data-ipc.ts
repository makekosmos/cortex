import { mkdirSync } from "node:fs";
import type { WebContents } from "electron";
import { isJsonString, type UserDataCallResult } from "./host-api";

export type { UserDataCallResult } from "./host-api";

type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

export const USER_DATA_MAX_BYTES = 25 * 1024 * 1024;

export type UserDataError = "not-found" | "invalid-key" | "too-large" | "io-error";
export type UserDataResult<T> = { ok: true; data: T } | { ok: false; error: UserDataError };
export type UserDataStat = { sizeBytes: number };
export type UserDataReadResult = UserDataResult<Uint8Array>;
export type UserDataWriteResult = UserDataResult<{ sizeBytes: number }>;
export type UserDataDeleteResult = UserDataResult<boolean>;
export type UserDataStatResult = UserDataResult<UserDataStat>;

const USER_DATA_KEY_RE = /^[\w][\w.-]{0,255}$/;

export function validateUserDataKey(key: string): boolean {
  if (key.length === 0 || key.length > 512) return false;
  const normalized = key.replaceAll("\\", "/");
  if (normalized.startsWith("/") || /^[A-Za-z]:\//.test(normalized) || normalized.includes("\0"))
    return false;
  return normalized
    .split("/")
    .every((part) => part !== "." && part !== ".." && USER_DATA_KEY_RE.test(part));
}

function resultError(error: UserDataError): UserDataResult<never> {
  return { ok: false, error };
}

/**
 * Binary user-data primitive implemented by Engine over its handle-relative
 * native filesystem. `openRoot` pins the Host userData base directory once;
 * every operation resolves `extension-data/<appId>/<key>` relative to that
 * handle, so a junction or replaced parent can never redirect I/O outside it.
 * A reported `unknown-root` asks the caller to re-pin and retry once.
 */
export interface UserDataBackend {
  openRoot(root: string): Promise<UserDataCallResult<{ rootId: string }>>;
  read(rootId: string, appId: string, key: string): Promise<UserDataCallResult<Uint8Array>>;
  write(
    rootId: string,
    appId: string,
    key: string,
    bytes: Uint8Array,
  ): Promise<UserDataCallResult<{ sizeBytes: number }>>;
  stat(
    rootId: string,
    appId: string,
    key: string,
  ): Promise<UserDataCallResult<{ sizeBytes: number }>>;
  delete(
    rootId: string,
    appId: string,
    key: string,
  ): Promise<UserDataCallResult<{ deleted: true }>>;
}

export interface UserDataStore {
  read(key: string): Promise<UserDataReadResult>;
  write(key: string, bytes: Uint8Array): Promise<UserDataWriteResult>;
  delete(key: string): Promise<UserDataDeleteResult>;
  stat(key: string): Promise<UserDataStatResult>;
}

export interface UserDataStores {
  forApp(appId: string): UserDataStore;
}

const SAFE_ROOT_ID = /^[\w-]{1,128}$/;
const SAFE_APP_ID = /^[\w][\w.-]{0,255}$/;

function mapEngineError(code: string): UserDataError {
  return code === "not-found" || code === "invalid-key" || code === "too-large" ? code : "io-error";
}

export function createEngineUserDataStores(
  backend: UserDataBackend,
  userDataRoot: string,
): UserDataStores {
  let pinned: Promise<UserDataResult<string>> | null = null;

  const pin = (): Promise<UserDataResult<string>> => {
    if (pinned) return pinned;
    const pending = backend.openRoot(userDataRoot).then(async (result) => {
      if (!result.ok && result.error === "not-found") {
        try {
          mkdirSync(userDataRoot, { recursive: true });
        } catch {
          return resultError("io-error");
        }
        result = await backend.openRoot(userDataRoot);
      }
      if (!result.ok) return resultError(mapEngineError(result.error));
      return SAFE_ROOT_ID.test(result.data.rootId)
        ? ({ ok: true, data: result.data.rootId } as const)
        : resultError("io-error");
    });
    pinned = pending;
    void pending.then((settled) => {
      if (!settled.ok && pinned === pending) pinned = null;
    });
    return pending;
  };

  const invoke = async <T>(
    appId: string,
    call: (rootId: string) => Promise<UserDataCallResult<T>>,
  ): Promise<UserDataResult<T>> => {
    if (!SAFE_APP_ID.test(appId)) return resultError("io-error");
    let root = await pin();
    if (!root.ok) return root;
    let result = await call(root.data);
    if (!result.ok && result.error === "unknown-root") {
      pinned = null;
      root = await pin();
      if (!root.ok) return root;
      result = await call(root.data);
    }
    return result.ok ? result : resultError(mapEngineError(result.error));
  };

  const stores = new Map<string, UserDataStore>();

  return {
    forApp(appId: string): UserDataStore {
      let store = stores.get(appId);
      if (store) return store;
      store = {
        read(key) {
          if (!validateUserDataKey(key)) return Promise.resolve(resultError("invalid-key"));
          return invoke(appId, (rootId) => backend.read(rootId, appId, key)).then((result) =>
            result.ok && result.data.byteLength > USER_DATA_MAX_BYTES
              ? resultError("too-large")
              : result,
          );
        },
        write(key, bytes) {
          if (!(bytes instanceof Uint8Array)) return Promise.resolve(resultError("io-error"));
          if (bytes.byteLength > USER_DATA_MAX_BYTES)
            return Promise.resolve(resultError("too-large"));
          if (!validateUserDataKey(key)) return Promise.resolve(resultError("invalid-key"));
          return invoke(appId, (rootId) => backend.write(rootId, appId, key, bytes)).then(
            (result) =>
              result.ok && result.data.sizeBytes !== bytes.byteLength
                ? resultError("io-error")
                : result,
          );
        },
        delete(key) {
          if (!validateUserDataKey(key)) return Promise.resolve(resultError("invalid-key"));
          return invoke(appId, (rootId) => backend.delete(rootId, appId, key)).then((result) =>
            result.ok
              ? result.data.deleted === true
                ? ({ ok: true, data: true } as const)
                : resultError("io-error")
              : result,
          );
        },
        stat(key) {
          if (!validateUserDataKey(key)) return Promise.resolve(resultError("invalid-key"));
          return invoke(appId, (rootId) => backend.stat(rootId, appId, key)).then((result) => {
            if (!result.ok) return result;
            const sizeBytes = result.data.sizeBytes;
            return !Number.isInteger(sizeBytes) || sizeBytes < 0
              ? resultError("io-error")
              : sizeBytes > USER_DATA_MAX_BYTES
                ? resultError("too-large")
                : result;
          });
        },
      };
      stores.set(appId, store);
      return store;
    },
  };
}

export type UserDataRequest =
  | { operation: "read" | "stat" | "delete"; key: string }
  | { operation: "write"; key: string; bytes: Uint8Array };

export interface UserDataAppAuthority {
  appId: string;
  permissions: readonly string[];
}

export interface ExtensionUserDataIpcOptions {
  handle(channel: string, handler: UserDataIpcHandler): void;
  resolveAppForSender(sender: WebContents): UserDataAppAuthority | null;
  userDataStoreForApp(appId: string): UserDataStore;
}

export type UserDataIpcEnvelope = {
  operation?: JsonValue;
  key?: JsonValue;
  bytes?: Uint8Array;
};

function parseRequest(value: UserDataIpcEnvelope | null): UserDataRequest | null {
  if (value === null || !isJsonString(value.operation) || !isJsonString(value.key)) return null;
  if (value.operation === "write") {
    return value.bytes instanceof Uint8Array
      ? { operation: "write", key: value.key, bytes: value.bytes }
      : null;
  }
  if (value.operation === "read") return { operation: "read", key: value.key };
  if (value.operation === "stat") return { operation: "stat", key: value.key };
  if (value.operation === "delete") return { operation: "delete", key: value.key };
  return null;
}

export type UserDataIpcHandler = (
  event: { sender: WebContents },
  input: UserDataIpcEnvelope | null,
) => Promise<UserDataResult<unknown>>;

export function registerExtensionUserDataIpc({
  handle,
  resolveAppForSender,
  userDataStoreForApp,
}: ExtensionUserDataIpcOptions): void {
  handle("host:user-data:binary", async (event, input) => {
    const authority = resolveAppForSender(event.sender);
    if (!authority) throw new Error("Unknown app sender");
    const request = parseRequest(input);
    if (!request) throw new Error("Invalid binary user data request");
    const writing = request.operation === "write" || request.operation === "delete";
    const permission = writing ? "filesystem.write" : "filesystem.read";
    if (!authority.permissions.includes(permission)) throw new Error("User data permission denied");
    const store = userDataStoreForApp(authority.appId);
    if (request.operation === "read") return store.read(request.key);
    if (request.operation === "write") return store.write(request.key, request.bytes);
    if (request.operation === "delete") return store.delete(request.key);
    return store.stat(request.key);
  });
}
