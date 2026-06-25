import type { KeplerLockInfo } from "./ensure-kepler.js";
import type {
  PendingRequest,
  SidecarEvent,
  SidecarRequest,
  SidecarResponse,
} from "./ark-client.types.js";

const MAX_QUEUE_SIZE = 500;
const DEFAULT_REQUEST_TIMEOUT_MS = 30_000;

export interface ArkKeplerTransportOptions {
  lock: KeplerLockInfo;
  deviceId: string;
  pidForHandshake?: number;
  requestTimeoutMs?: number;
  onEvent: (event: SidecarEvent) => void;
}

export class ArkKeplerTransport {
  private keplerWs: WebSocket | null = null;
  private keplerHandshakeDone = false;
  private keplerConnectPromise: Promise<void> | null = null;
  private pendingRequests: Map<string, PendingRequest<unknown>> = new Map();
  private nextRequestSeq = 0;

  constructor(private readonly opts: ArkKeplerTransportOptions) {}

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

  // -------------------------------------------------------------------------
  // Internal: Kepler mode (WebSocket → Kepler host)
  // -------------------------------------------------------------------------

  async ensureConnected(): Promise<void> {
    if (this.keplerHandshakeDone && this.keplerWs && this.keplerWs.readyState === WebSocket.OPEN) {
      return;
    }
    if (this.keplerConnectPromise) {
      return this.keplerConnectPromise;
    }
    this.keplerConnectPromise = this.openConnection();
    try {
      await this.keplerConnectPromise;
    } finally {
      this.keplerConnectPromise = null;
    }
  }

  private async openConnection(): Promise<void> {
    const lock = this.opts.lock;
    if (!lock) {
      throw new Error("@kosmos/ark: keplerLock is required for kepler mode");
    }

    const url = `ws://127.0.0.1:${lock.ws_port}`;
    const ws = new WebSocket(url);

    await new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        ws.removeEventListener("error", onError as never);
        resolve();
      };
      const onError = (_e: Event) => {
        ws.removeEventListener("open", onOpen as never);
        reject(new Error(`Kepler WS open failed at ${url}`));
      };
      ws.addEventListener("open", onOpen, { once: true });
      ws.addEventListener("error", onError, { once: true });
    });

    const protocolVersionString = `${lock.protocol_version.major}.${lock.protocol_version.minor}.${lock.protocol_version.patch}`;
    const helloPayload = {
      kind: "hello",
      protocolVersion: protocolVersionString,
      token: lock.auth_token,
      pid: this.opts.pidForHandshake ?? process.pid,
      clientId: this.opts.deviceId,
    };
    ws.send(JSON.stringify(helloPayload));

    const helloResponse = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const onMessage = (evt: MessageEvent) => {
        ws.removeEventListener("message", onMessage as never);
        try {
          resolve(JSON.parse(String(evt.data)) as Record<string, unknown>);
        } catch (e) {
          reject(e instanceof Error ? e : new Error(String(e)));
        }
      };
      ws.addEventListener("message", onMessage, { once: true });
    });

    if (helloResponse.kind !== "hello_ok") {
      ws.close();
      const code = String(helloResponse.code ?? "unknown");
      const message = String(helloResponse.message ?? "");
      throw new Error(`Kepler rejected handshake (${code}): ${message}`);
    }

    // Ongoing message handler — обрабатывает все frames после hello_ok.
    ws.addEventListener("message", (evt) => {
      this.handleKeplerFrame(String(evt.data));
    });
    ws.addEventListener("close", () => {
      this.keplerWs = null;
      this.keplerHandshakeDone = false;
      // Pending requests fail с явной ошибкой; апка должна implement reconnect
      // через ensureKeplerRunning + новый ArkClient. AC2 на этом уровне.
      const error = new Error("Kepler connection closed");
      for (const pending of this.pendingRequests.values()) {
        clearTimeout(pending.timeout);
        pending.reject(error);
      }
      this.pendingRequests.clear();
    });
    ws.addEventListener("error", () => {
      // 'close' followed обычно — single handler. Здесь — просто log/noop.
    });

    this.keplerWs = ws;
    this.keplerHandshakeDone = true;
  }

  private handleKeplerFrame(text: string): void {
    if (!text.trim()) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      return;
    }
    const obj = parsed as Record<string, unknown>;

    // Async events (no `ok`).
    if ("event" in obj && !("ok" in obj)) {
      this.opts.onEvent(obj as unknown as SidecarEvent);
      return;
    }

    const resp = obj as unknown as SidecarResponse<unknown>;
    const respReqId =
      typeof obj._req_id === "string"
        ? (obj._req_id as string)
        : typeof obj.id === "string"
          ? (obj.id as string)
          : undefined;
    const pending = this.takePendingRequest(respReqId);
    if (!pending) return;

    if (!resp.ok) {
      pending.reject(new Error(resp.error ?? "kepler error"));
    } else {
      pending.resolve(resp.data);
    }
  }

  async request<T>(req: SidecarRequest): Promise<T> {
    await this.ensureConnected();
    const ws = this.keplerWs;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      throw new Error("Kepler WS not connected");
    }
    if (this.pendingRequests.size >= MAX_QUEUE_SIZE) {
      return Promise.reject(new Error(`@kosmos/ark request queue overflow (${MAX_QUEUE_SIZE})`));
    }

    const id = this.makeRequestId();
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
      ws.send(JSON.stringify(request));
    } catch (err) {
      const pending = this.pendingRequests.get(id);
      if (pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(id);
      }
      return Promise.reject(err instanceof Error ? err : new Error(String(err)));
    }

    return promise;
  }

  close(): void {
    if (this.keplerWs) {
      try {
        this.keplerWs.close();
      } catch {
        // ignore
      }
      this.keplerWs = null;
    }
    this.keplerHandshakeDone = false;
    for (const pending of this.pendingRequests.values()) {
      clearTimeout(pending.timeout);
    }
    this.pendingRequests.clear();
  }
}
