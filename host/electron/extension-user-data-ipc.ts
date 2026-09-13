import { randomUUID } from "node:crypto";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import type { WebContents } from "electron";
import { isJsonString } from "./host-api";

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

export function createUserDataStore(root: string) {
  const filePath = (key: string): string | null => {
    if (!validateUserDataKey(key)) return null;
    const normalized = key.replaceAll("\\", "/");
    const resolvedRoot = path.resolve(root);
    const resolved = path.resolve(resolvedRoot, ...normalized.split("/"));
    const relative = path.relative(resolvedRoot, resolved);
    return relative.startsWith("..") || path.isAbsolute(relative) ? null : resolved;
  };

  const readStat = async (key: string): Promise<UserDataStatResult> => {
    const file = filePath(key);
    if (!file) return resultError("invalid-key");
    try {
      const details = await stat(file);
      if (!details.isFile()) return resultError("io-error");
      if (details.size > USER_DATA_MAX_BYTES) return resultError("too-large");
      return { ok: true, data: { sizeBytes: details.size } };
    } catch (error) {
      return resultError(
        error instanceof Error && "code" in error && error.code === "ENOENT"
          ? "not-found"
          : "io-error",
      );
    }
  };

  return {
    async read(key: string): Promise<UserDataReadResult> {
      const details = await readStat(key);
      if (!details.ok) return details;
      const file = filePath(key);
      if (!file) return resultError("invalid-key");
      try {
        const bytes = await readFile(file);
        return bytes.byteLength > USER_DATA_MAX_BYTES
          ? resultError("too-large")
          : { ok: true, data: new Uint8Array(bytes) };
      } catch (error) {
        return resultError(
          error instanceof Error && "code" in error && error.code === "ENOENT"
            ? "not-found"
            : "io-error",
        );
      }
    },

    async write(key: string, bytes: Uint8Array): Promise<UserDataWriteResult> {
      const file = filePath(key);
      if (!file) return resultError("invalid-key");
      if (!(bytes instanceof Uint8Array)) return resultError("io-error");
      if (bytes.byteLength > USER_DATA_MAX_BYTES) return resultError("too-large");
      const temporary = `${file}.${randomUUID()}.tmp`;
      try {
        await mkdir(path.dirname(file), { recursive: true });
        await writeFile(temporary, Buffer.from(bytes), { flag: "wx" });
        await rename(temporary, file);
        return { ok: true, data: { sizeBytes: bytes.byteLength } };
      } catch {
        return resultError("io-error");
      } finally {
        await rm(temporary, { force: true }).catch(() => undefined);
      }
    },

    async delete(key: string): Promise<UserDataDeleteResult> {
      const file = filePath(key);
      if (!file) return resultError("invalid-key");
      const details = await readStat(key);
      if (!details.ok) return details.error === "not-found" ? details : resultError(details.error);
      try {
        await rm(file);
        return { ok: true, data: true };
      } catch (error) {
        return resultError(
          error instanceof Error && "code" in error && error.code === "ENOENT"
            ? "not-found"
            : "io-error",
        );
      }
    },

    stat: readStat,
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
  userDataDirForApp(appId: string): string;
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
  userDataDirForApp,
}: ExtensionUserDataIpcOptions): void {
  handle("host:user-data:binary", async (event, input) => {
    const authority = resolveAppForSender(event.sender);
    if (!authority) throw new Error("Unknown app sender");
    const request = parseRequest(input);
    if (!request) throw new Error("Invalid binary user data request");
    const writing = request.operation === "write" || request.operation === "delete";
    const permission = writing ? "filesystem.write" : "filesystem.read";
    if (!authority.permissions.includes(permission)) throw new Error("User data permission denied");
    const store = createUserDataStore(userDataDirForApp(authority.appId));
    if (request.operation === "read") return store.read(request.key);
    if (request.operation === "write") return store.write(request.key, request.bytes);
    if (request.operation === "delete") return store.delete(request.key);
    return store.stat(request.key);
  });
}
