import { app } from "electron";
import { ensureEngineRunning, type EngineLockInfo } from "@kosmos/ark";
import {
  ReconnectingEngineClient,
  type ReconnectingEngineTransport,
} from "@kosmos/ark/reconnecting-engine-client";
import { randomUUID } from "node:crypto";
import type { ManagerErrorCode, ManagerResult } from "../src/manager-api";

type Json = Record<string, unknown>;
const ENGINE_API_VERSION = "1.0.0";
let dictationEventListener: ((event: unknown) => void) | null = null;

class EngineHttpTransport implements ReconnectingEngineTransport {
  constructor(readonly lock: EngineLockInfo) {}
  async start(): Promise<void> {}
  async stop(): Promise<void> {}
  async invokeOperation<T>(): Promise<T> {
    throw new Error("Manager HTTP transport does not support ARK operations");
  }
  onArkEvent(): () => void {
    return () => {};
  }
}

const engine = new ReconnectingEngineClient<EngineHttpTransport>({
  discover: () =>
    ensureEngineRunning({
      appDataPath: app.getPath("appData"),
      dataDir: process.env.KOSMOS_DATA_DIR,
      clientProtocolMajor: 1,
      autoLaunch: true,
    }),
  createTransport: (lock) => new EngineHttpTransport(lock),
});

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
    const transport = await engine.getTransport();
    const { api_version: version } = transport.lock;
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
  return engine.isConnected();
}

export function subscribeDictationEvents(listener: (event: unknown) => void): () => void {
  dictationEventListener = listener;
  const stop = engine.onArkEvent((event) => {
    if (typeof event.event === "string") dictationEventListener?.(event);
  });
  return () => {
    if (dictationEventListener === listener) {
      dictationEventListener = null;
      stop();
    }
  };
}

export async function disconnectEngine(): Promise<void> {
  await engine.stop();
}

export async function rpc(operation: string, params: Json = {}): Promise<ManagerResult<unknown>> {
  try {
    return await engine.execute(async ({ lock }) => {
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
    });
  } catch (error) {
    return mapError(error);
  }
}

export async function status(pathname: "/v1/health" | "/v1/info"): Promise<ManagerResult<unknown>> {
  try {
    return await engine.execute(
      async ({ lock }) => {
        const response = await fetch(`http://127.0.0.1:${lock.http_port}${pathname}`, {
          headers: headers(lock),
          signal: AbortSignal.timeout(8_000),
        });
        rejectRecoverable(response);
        const value = (await response.json()) as unknown;
        return response.ok ? { ok: true, data: value } : fail("engine", "Engine вернул ошибку.");
      },
      { idempotent: true },
    );
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
