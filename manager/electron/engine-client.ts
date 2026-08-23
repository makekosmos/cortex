import { app } from "electron";
import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import type { ManagerErrorCode, ManagerResult } from "../src/manager-api";

type Json = Record<string, unknown>;
const ENGINE_API_VERSION = "1.0.0";
let dictationEventListener: ((event: unknown) => void) | null = null;

interface EngineLockInfo {
  format_version: number;
  api_version: { major: number; minor: number; patch: number };
  pid: number;
  http_port: number;
  auth_token: string;
}

function readEngineLock(): EngineLockInfo {
  const dataDir = process.env.KOSMOS_DATA_DIR || path.join(app.getPath("appData"), "Kosmos");
  const lockPath = path.join(dataDir, "engine.lock.json");
  let value: unknown;
  try {
    value = JSON.parse(fs.readFileSync(lockPath, "utf8"));
  } catch {
    throw new Error("Engine lock недоступен");
  }
  if (!isEngineLock(value)) throw new Error("Engine lock повреждён");
  if (value.api_version.major !== 1) throw new Error("Engine version incompatible");
  return value;
}

function isEngineLock(value: unknown): value is EngineLockInfo {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const lock = value as Record<string, unknown>;
  const version = lock.api_version;
  return (
    lock.format_version === 1 &&
    typeof lock.pid === "number" &&
    typeof lock.http_port === "number" &&
    lock.http_port > 0 &&
    typeof lock.auth_token === "string" &&
    Boolean(lock.auth_token) &&
    Boolean(version) &&
    typeof version === "object" &&
    !Array.isArray(version) &&
    typeof (version as Record<string, unknown>).major === "number" &&
    typeof (version as Record<string, unknown>).minor === "number" &&
    typeof (version as Record<string, unknown>).patch === "number"
  );
}

function fail(code: ManagerErrorCode, message: string): ManagerResult<never> {
  return { ok: false, code, message };
}
function mapError(error: unknown): ManagerResult<never> {
  const text = error instanceof Error ? error.message : String(error);
  if (/incompatible|version|версия|несовместима|API v1/i.test(text))
    return fail("incompatible_api", "Версия Engine несовместима. Обновите Kosmos.");
  if (/discovery|lock|not installed|spawn|Engine не установлен|устаревшее|повреждено/i.test(text))
    return fail("engine_unavailable", "Engine недоступен. Запустите Engine и повторите попытку.");
  return fail("transport", "Не удалось связаться с Engine. Повторите попытку.");
}

function headers(lock: EngineLockInfo): HeadersInit {
  return {
    Authorization: `Bearer ${lock.auth_token}`,
    "Content-Type": "application/json",
    "X-Kosmos-Api-Version": ENGINE_API_VERSION,
    "X-Kosmos-Client-Class": "engine-manager",
    "X-Kosmos-Client-Version": app.getVersion(),
    "X-Kosmos-Client-Pid": String(process.pid),
  };
}

function rejectRecoverable(response: Response): void {
  if ([401, 403, 426].includes(response.status)) {
    throw Object.assign(new Error(`HTTP ${response.status}`), {
      status: response.status,
    });
  }
}

export async function connectEngine(): Promise<ManagerResult<{ apiVersion: string }>> {
  try {
    const { api_version: version } = readEngineLock();
    return {
      ok: true,
      data: {
        apiVersion: `${version.major}.${version.minor}.${version.patch}`,
      },
    };
  } catch (error) {
    return mapError(error);
  }
}

export function engineConnected(): boolean {
  try {
    readEngineLock();
    return true;
  } catch {
    return false;
  }
}

export function subscribeDictationEvents(listener: (event: unknown) => void): () => void {
  dictationEventListener = listener;
  return () => {
    if (dictationEventListener === listener) {
      dictationEventListener = null;
    }
  };
}

export async function disconnectEngine(): Promise<void> {
  dictationEventListener = null;
}

export async function rpc(operation: string, params: Json = {}): Promise<ManagerResult<unknown>> {
  try {
    const lock = readEngineLock();
    return await (async () => {
      const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
        method: "POST",
        headers: headers(lock),
        body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
        signal: AbortSignal.timeout(12_000),
      });
      rejectRecoverable(response);
      const value = (await response.json()) as {
        ok?: boolean;
        data?: unknown;
        error?: string;
      };
      if (!response.ok || value.ok === false)
        return fail("engine", value.error ? "Engine отклонил операцию." : "Engine вернул ошибку.");
      return { ok: true, data: value.data ?? value };
    })();
  } catch (error) {
    return mapError(error);
  }
}

export async function status(pathname: "/v1/health" | "/v1/info"): Promise<ManagerResult<unknown>> {
  try {
    const lock = readEngineLock();
    return await (async () => {
        const response = await fetch(`http://127.0.0.1:${lock.http_port}${pathname}`, {
          headers: headers(lock),
          signal: AbortSignal.timeout(8_000),
        });
        rejectRecoverable(response);
        const value = (await response.json()) as unknown;
        return response.ok ? { ok: true, data: value } : fail("engine", "Engine вернул ошибку.");
    })();
  } catch (error) {
    return mapError(error);
  }
}

export async function waitForEngineReady(timeoutMs = 30_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await status("/v1/health")).ok) return;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}
