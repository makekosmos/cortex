import { randomUUID } from "node:crypto";
import { lstat, mkdir, open, realpath, rename, rm, stat, writeFile } from "node:fs/promises";
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
export type UserDataOperation = "read" | "write" | "delete" | "stat";
export interface UserDataStoreOptions {
  beforeAccess?: (operation: UserDataOperation, filePath: string) => void | Promise<void>;
}

function samePath(left: string, right: string): boolean {
  const a = path.normalize(left);
  const b = path.normalize(right);
  return process.platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b;
}

function isContained(root: string, candidate: string): boolean {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

export function createUserDataStore(root: string, options: UserDataStoreOptions = {}) {
  const resolvedRoot = path.resolve(root);
  const filePath = (key: string): string | null => {
    if (!validateUserDataKey(key)) return null;
    const normalized = key.replaceAll("\\", "/");
    const resolved = path.resolve(resolvedRoot, ...normalized.split("/"));
    const relative = path.relative(resolvedRoot, resolved);
    return relative.startsWith("..") || path.isAbsolute(relative) ? null : resolved;
  };

  const safeFilePath = async (
    key: string,
    createParents = false,
  ): Promise<UserDataResult<string>> => {
    const file = filePath(key);
    if (!file) return resultError("invalid-key");
    let realRoot: string;
    try {
      realRoot = await realpath(resolvedRoot);
    } catch (error) {
      return resultError(
        !createParents && error instanceof Error && "code" in error && error.code === "ENOENT"
          ? "not-found"
          : "io-error",
      );
    }
    if (!samePath(realRoot, resolvedRoot)) return resultError("io-error");
    let current = realRoot;
    const parts = key.replaceAll("\\", "/").split("/");
    for (const [index, part] of parts.entries()) {
      current = path.join(current, part);
      try {
        const entry = await lstat(current);
        const realPath = await realpath(current);
        if (
          entry.isSymbolicLink() ||
          !samePath(realPath, current) ||
          !isContained(realRoot, realPath)
        )
          return resultError("io-error");
      } catch (error) {
        if (
          error instanceof Error &&
          "code" in error &&
          error.code === "ENOENT" &&
          createParents &&
          index < parts.length - 1
        ) {
          try {
            await mkdir(current);
          } catch (mkdirError) {
            if (
              !(mkdirError instanceof Error) ||
              !("code" in mkdirError) ||
              mkdirError.code !== "EEXIST"
            )
              return resultError("io-error");
          }
          try {
            const entry = await lstat(current);
            const realPath = await realpath(current);
            if (
              entry.isSymbolicLink() ||
              !samePath(realPath, current) ||
              !isContained(realRoot, realPath)
            )
              return resultError("io-error");
          } catch {
            return resultError("io-error");
          }
          continue;
        }
        if (error instanceof Error && "code" in error && error.code === "ENOENT") {
          return createParents || index === parts.length - 1
            ? { ok: true, data: current }
            : resultError("not-found");
        }
        return resultError("io-error");
      }
    }
    return { ok: true, data: current };
  };

  const resolveForAccess = async (
    key: string,
    operation: UserDataOperation,
    createParents = false,
  ): Promise<UserDataResult<string>> => {
    const resolved = await safeFilePath(key, createParents);
    if (!resolved.ok) return resolved;
    try {
      await options.beforeAccess?.(operation, resolved.data);
    } catch {
      return resultError("io-error");
    }
    const verified = await safeFilePath(key, createParents);
    return verified.ok && samePath(verified.data, resolved.data)
      ? verified
      : resultError("io-error");
  };

  const readStat = async (key: string, operation: "stat" = "stat"): Promise<UserDataStatResult> => {
    const resolved = await resolveForAccess(key, operation);
    if (!resolved.ok) return resolved;
    try {
      const details = await stat(resolved.data);
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
      const resolved = await resolveForAccess(key, "read");
      if (!resolved.ok) return resolved;
      let file: Awaited<ReturnType<typeof open>> | undefined;
      try {
        file = await open(resolved.data, "r");
        const details = await file.stat();
        if (!details.isFile()) return resultError("io-error");
        if (details.size > USER_DATA_MAX_BYTES) return resultError("too-large");
        const buffer = Buffer.allocUnsafe(USER_DATA_MAX_BYTES + 1);
        const { bytesRead } = await file.read(buffer, 0, USER_DATA_MAX_BYTES + 1, 0);
        return bytesRead > USER_DATA_MAX_BYTES
          ? resultError("too-large")
          : { ok: true, data: new Uint8Array(buffer.subarray(0, bytesRead)) };
      } catch (error) {
        return resultError(
          error instanceof Error && "code" in error && error.code === "ENOENT"
            ? "not-found"
            : "io-error",
        );
      } finally {
        await file?.close().catch(() => undefined);
      }
    },

    async write(key: string, bytes: Uint8Array): Promise<UserDataWriteResult> {
      if (!(bytes instanceof Uint8Array)) return resultError("io-error");
      if (bytes.byteLength > USER_DATA_MAX_BYTES) return resultError("too-large");
      try {
        await mkdir(resolvedRoot, { recursive: true });
      } catch {
        return resultError("io-error");
      }
      const resolved = await resolveForAccess(key, "write", true);
      if (!resolved.ok) return resolved;
      const file = resolved.data;
      const temporary = `${file}.${randomUUID()}.tmp`;
      try {
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
      const resolved = await resolveForAccess(key, "delete");
      if (!resolved.ok) return resolved;
      try {
        await rm(resolved.data);
        return { ok: true, data: true };
      } catch (error) {
        return resultError(
          error instanceof Error && "code" in error && error.code === "ENOENT"
            ? "not-found"
            : "io-error",
        );
      }
    },

    stat: (key: string) => readStat(key),
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
