import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";

import type {
  PendingRequest,
  SidecarEvent,
  SidecarRequest,
  SidecarResponse,
} from "./ark-client.types.js";

const MAX_QUEUE_SIZE = 500;
const DEFAULT_REQUEST_TIMEOUT_MS = 30_000;

export interface ArkChildTransportOptions {
  sidecarPath?: string;
  requestTimeoutMs?: number;
  onEvent: (event: SidecarEvent) => void;
}

export class ArkChildTransport {
  private child: ChildProcessWithoutNullStreams | null = null;
  private stdoutChunks: Buffer[] = [];
  private stdoutLength = 0;
  private stderrBuffer = "";
  private pendingRequests: Map<string, PendingRequest<unknown>> = new Map();
  private nextRequestSeq = 0;

  constructor(private readonly opts: ArkChildTransportOptions) {}

  private ensureChild(): ChildProcessWithoutNullStreams {
    if (this.child) return this.child;

    const binaryPath = this.opts.sidecarPath;
    if (!binaryPath)
      throw new Error("@kosmos/ark: sidecarPath is required when requestFn is not provided");

    const child = spawn(binaryPath, [], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      this.stderrBuffer += chunk;
    });

    child.stdout.on("data", (chunk: Buffer | string) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      this.stdoutChunks.push(buf);
      this.stdoutLength += buf.length;
      this.flushStdout();
    });

    child.on("error", (err) => {
      this.failAll(err instanceof Error ? err : new Error(String(err)));
    });

    child.on("close", (code) => {
      const reason = this.stderrBuffer.trim() || `ark-core-rpc exited with ${code}`;
      this.failAll(new Error(reason));
    });

    this.child = child;
    this.stdoutChunks = [];
    this.stdoutLength = 0;
    this.stderrBuffer = "";

    return child;
  }

  kill(): void {
    if (this.child) {
      this.child.stdout.removeAllListeners();
      this.child.stderr.removeAllListeners();
      this.child.removeAllListeners();
      if (!this.child.killed) this.child.kill();
      this.child = null;
    }
    this.stdoutChunks = [];
    this.stdoutLength = 0;
    this.stderrBuffer = "";
    for (const pending of this.pendingRequests.values()) {
      clearTimeout(pending.timeout);
    }
    this.pendingRequests.clear();
  }

  private flushStdout(): void {
    const merged = Buffer.concat(this.stdoutChunks, this.stdoutLength);
    this.stdoutChunks = [];
    this.stdoutLength = 0;

    let searchFrom = 0;
    while (true) {
      const nl = merged.indexOf(0x0a, searchFrom);
      if (nl === -1) {
        if (searchFrom < merged.length) {
          const remaining = merged.subarray(searchFrom);
          this.stdoutChunks.push(remaining);
          this.stdoutLength = remaining.length;
        }
        return;
      }

      const rawLine = merged.subarray(searchFrom, nl).toString("utf8").trim();
      searchFrom = nl + 1;

      if (!rawLine) continue;

      let parsed: unknown;
      try {
        parsed = JSON.parse(rawLine);
      } catch (err) {
        this.failAll(err instanceof Error ? err : new Error(String(err)));
        continue;
      }

      const obj = parsed as Record<string, unknown>;

      // Async events have `event` field, no `ok` field.
      if ("event" in obj && !("ok" in obj)) {
        this.opts.onEvent(obj as unknown as SidecarEvent);
        continue;
      }

      const resp = obj as unknown as SidecarResponse<unknown>;
      // Rust echoes envelope-id обратно как `_req_id` (новое) или `id` (legacy).
      const respReqId =
        typeof obj._req_id === "string"
          ? (obj._req_id as string)
          : typeof obj.id === "string"
            ? (obj.id as string)
            : undefined;
      const pending = this.takePendingRequest(respReqId);
      if (!pending) continue;

      if (!resp.ok) {
        pending.reject(new Error(resp.error ?? "ark-core-rpc error"));
      } else {
        pending.resolve(resp.data);
      }
    }
  }

  private takePendingRequest(id: string | undefined): PendingRequest<unknown> | null {
    if (id) {
      const pending = this.pendingRequests.get(id);
      if (!pending) return null;
      this.pendingRequests.delete(id);
      clearTimeout(pending.timeout);
      return pending;
    }

    // Backward-compatible fallback for old sidecars that do not echo ids.
    const first = this.pendingRequests.keys().next();
    if (first.done) return null;
    const fallbackId = first.value;
    const pending = this.pendingRequests.get(fallbackId) ?? null;
    this.pendingRequests.delete(fallbackId);
    if (pending) clearTimeout(pending.timeout);
    return pending;
  }

  private makeRequestId(): string {
    this.nextRequestSeq += 1;
    return `kosmos-ark-${Date.now()}-${this.nextRequestSeq}`;
  }

  private sendRequest<T>(req: SidecarRequest): Promise<T> {
    if (this.pendingRequests.size >= MAX_QUEUE_SIZE) {
      return Promise.reject(new Error(`@kosmos/ark request queue overflow (${MAX_QUEUE_SIZE})`));
    }

    const child = this.ensureChild();
    const id = this.makeRequestId();
    // ВАЖНО: envelope-id живёт в `_req_id`, не в `id`. Иначе он бы затирал
    // payload-поле `id` у операций вроде get_object / delete_object — это
    // приводит к молчаливому "not found", потому что Rust пытался искать
    // строку запроса в БД. Rust обрабатывает оба поля для обратной совместимости.
    const request = { ...req, _req_id: id };
    const timeoutMs = this.opts.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS;

    const promise = new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const pending = this.pendingRequests.get(id);
        if (!pending) return;
        this.pendingRequests.delete(id);
        reject(new Error(`@kosmos/ark request timed out after ${timeoutMs}ms: ${req.operation}`));
      }, timeoutMs);
      this.pendingRequests.set(id, {
        id,
        request,
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      });
    });

    try {
      child.stdin.write(`${JSON.stringify(request)}\n`);
    } catch (err) {
      const pending = this.pendingRequests.get(id);
      if (pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(id);
      }
      this.kill();
      return Promise.reject(err instanceof Error ? err : new Error(String(err)));
    }

    return promise;
  }

  private failAll(error: Error): void {
    const pending = Array.from(this.pendingRequests.values());
    this.pendingRequests.clear();
    this.child = null;
    pending.forEach((r) => {
      clearTimeout(r.timeout);
      r.reject(error);
    });
  }

  request<T>(req: SidecarRequest): Promise<T> {
    return this.sendRequest<T>(req);
  }
}
