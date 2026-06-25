import fs from "node:fs";
import path from "node:path";

import { ArkChildTransport } from "./ark-child-transport.js";
import { ArkKeplerTransport } from "./ark-kepler-transport.js";

import { createArkClientApis } from "./ark-client-apis.js";
import { generateHlc, withLocalWriteDeviceId } from "./ark-client-helpers.js";
import { dispatchArkEvent } from "./ark-event-dispatcher.js";

import type {
  ArkClientOptions,
  SidecarEvent,
  ConnectedPeer,
  SidecarRequest,
  PeerConnectedCallback,
  PeerDisconnectedCallback,
  EntityChangedCallback,
  CommandInvokedCallback,
  CommandsChangedCallback,
  ArkObjectsApi,
  ArkObjectTypesApi,
  ArkLinksApi,
  ArkKvApi,
  ArkCommandsApi,
} from "./ark-client.types.js";

import type { ArkUsageApi } from "./ark-client-usage.types.js";

// ---------------------------------------------------------------------------
// ArkClient
// ---------------------------------------------------------------------------

export class ArkClient {
  private readonly opts: ArkClientOptions;
  readonly objects: ArkObjectsApi;
  readonly objectTypes: ArkObjectTypesApi;
  readonly links: ArkLinksApi;
  readonly usage: ArkUsageApi;
  readonly kv: ArkKvApi;
  readonly commands: ArkCommandsApi;

  // ---- Built-in sidecar child-process (used when requestFn is absent) ----
  private childTransport: ArkChildTransport | null = null;

  // ---- Injected sidecar delegate ----
  private readonly delegateRequest: (<T>(req: Record<string, unknown>) => Promise<T>) | null;
  private delegateUnsubscribe: (() => void) | null = null;

  // ---- Kepler mode (Phase 2) ----
  private keplerTransport: ArkKeplerTransport | null = null;
  private initialized = false;

  // ---- Event callbacks ----
  private peerConnectedCallbacks: Set<PeerConnectedCallback> = new Set();
  private peerDisconnectedCallbacks: Set<PeerDisconnectedCallback> = new Set();
  private entityChangedCallbacks: Set<EntityChangedCallback> = new Set();
  private arkEventCallbacks: Set<(event: SidecarEvent) => void> = new Set();
  private commandInvokedCallbacks: Set<CommandInvokedCallback> = new Set();
  private commandsChangedCallbacks: Set<CommandsChangedCallback> = new Set();

  constructor(opts: ArkClientOptions) {
    this.opts = opts;
    this.delegateRequest = opts.requestFn ?? null;
    const apis = createArkClientApis({
      deviceId: opts.deviceId,
      requestAfterInit: <T>(request: SidecarRequest) => this.requestAfterInit<T>(request),
      commandInvokedCallbacks: this.commandInvokedCallbacks,
      commandsChangedCallbacks: this.commandsChangedCallbacks,
    });
    this.objects = apis.objects;
    this.objectTypes = apis.objectTypes;
    this.links = apis.links;
    this.usage = apis.usage;
    this.kv = apis.kv;
    this.commands = apis.commands;
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  async start(): Promise<void> {
    await this.ensureInitialized();

    // Subscribe to events from the injected delegate or from our own child.
    if (this.opts.onEventFn) {
      this.delegateUnsubscribe = this.opts.onEventFn((event) => {
        this.dispatchSidecarEvent(event);
      });
    }

    // Architectural change: sync — opt-in feature, активируется только когда
    // запущен Kepler host (Kepler сам вызывает start_sync). Standalone-apps
    // (без Kepler installed) работают как локальные DB clients без LAN/relay
    // sync. Это позволяет single-machine usage без overhead Kepler installer'а.
    //
    // Apps что explicitly хотят standalone sync (rare case — например, для
    // тестов или специальных deployments) — могут вызвать `startSyncLegacy()`
    // напрямую.
  }

  /**
   * Legacy explicit `start_sync` для apps, которые хотят запустить sync без
   * Kepler host'а. В обычном flow — Kepler владеет sync, apps его не дёргают.
   */
  async startSyncLegacy(): Promise<void> {
    if (this.opts.keplerLock) {
      // Kepler уже владеет sync — повторный start_sync только нарушит state.
      return;
    }
    await this.request<boolean>({
      operation: "start_sync",
      space_id: this.opts.spaceId,
      device_id: this.opts.deviceId,
      device_name: this.opts.deviceName ?? null,
      port: this.opts.port ?? null,
      seed_addresses: null,
      relay_url: this.opts.relayUrl ?? null,
      relay_api_key: this.opts.relayApiKey ?? null,
      auth_secret: this.opts.authSecret ?? null,
      use_iroh: this.opts.useIroh ?? false,
      iroh_peer_ticket: this.opts.irohPeerTicket ?? null,
    });
  }

  /**
   * Step 4a: fetch our iroh pairing ticket from the running sidecar, so the
   * runtime/UI can surface it for pairing (e.g. show a QR/copy-paste code).
   * Returns `null` when sync isn't running, a different transport was
   * selected, or the sidecar build lacks `iroh-spike`.
   */
  async getOwnIrohTicket(): Promise<string | null> {
    return this.request<string | null>({ operation: "get_own_iroh_ticket" });
  }

  async stop(): Promise<void> {
    try {
      if (!this.opts.keplerLock) {
        // В kepler-режиме stop_sync владеет Kepler, апка не имеет права его дёргать.
        await this.request<boolean>({ operation: "stop_sync" });
      }
    } catch {
      // Ignore errors on stop
    }
    this.delegateUnsubscribe?.();
    this.delegateUnsubscribe = null;
    if (this.opts.keplerLock) {
      this.keplerTransport?.close();
      this.keplerTransport = null;
    } else if (!this.delegateRequest) {
      this.childTransport?.kill();
      this.childTransport = null;
    }
  }

  // -------------------------------------------------------------------------
  // Sync operations
  // -------------------------------------------------------------------------

  async broadcastChange(
    entityType: string,
    entityId: string,
    data: Record<string, unknown>,
    deleted = false,
  ): Promise<void> {
    await this.request<boolean>({
      operation: "broadcast_change",
      entity: {
        type: entityType,
        id: entityId,
        data,
        hlc: generateHlc(this.opts.deviceId),
        ...(deleted ? { deleted: true } : {}),
      },
    });
  }

  async getConnectedPeers(): Promise<Array<{ device_id: string; device_name: string }>> {
    return this.request<ConnectedPeer[]>({ operation: "get_connected_peers" });
  }

  /**
   * Public escape-hatch: прямой проход в ARK ops без typed wrapper. Используется
   * legacy callsites (Eden's main/store.ts) на время Phase 2 cutover — там
   * сотни вызовов через `runArkRequest({operation: "...", ...})`, переписывать
   * всё на typed API одновременно с переключением транспорта — слишком большой
   * blast radius. Когда Eden постепенно мигрирует на `client.objects.list()`
   * etc. — этот метод можно будет deprecate.
   *
   * Гарантирует, что connection (kepler или self-managed child) инициализирован.
   */
  async invokeOperation<T>(req: { operation: string; [key: string]: unknown }): Promise<T> {
    return this.requestAfterInit<T>(
      withLocalWriteDeviceId(req as SidecarRequest, this.opts.deviceId),
    );
  }

  // -------------------------------------------------------------------------
  // Event subscriptions (return unsubscribe function)
  // -------------------------------------------------------------------------

  onPeerConnected(cb: PeerConnectedCallback): () => void {
    this.peerConnectedCallbacks.add(cb);
    return () => {
      this.peerConnectedCallbacks.delete(cb);
    };
  }

  onPeerDisconnected(cb: PeerDisconnectedCallback): () => void {
    this.peerDisconnectedCallbacks.add(cb);
    return () => {
      this.peerDisconnectedCallbacks.delete(cb);
    };
  }

  onEntityChanged(cb: EntityChangedCallback): () => void {
    this.entityChangedCallbacks.add(cb);
    return () => {
      this.entityChangedCallbacks.delete(cb);
    };
  }

  /**
   * Generic event subscription — получает все события от ark-core-rpc (любого
   * `event` field). Используется для Phase 2 sync_error / sync_replay events,
   * у которых нет типизированного callback'а. Apps подписываются и фильтруют:
   *
   *   client.onArkEvent(e => {
   *     if (e.event === "sync_error") { ... }
   *   })
   *
   * Возвращает unsubscribe.
   */
  onArkEvent(cb: (event: SidecarEvent) => void): () => void {
    this.arkEventCallbacks.add(cb);
    return () => {
      this.arkEventCallbacks.delete(cb);
    };
  }

  // -------------------------------------------------------------------------
  // Internal: request routing
  // -------------------------------------------------------------------------

  private request<T>(req: SidecarRequest): Promise<T> {
    if (this.delegateRequest) {
      // Use the injected request function (external sidecar management).
      return this.delegateRequest<T>(req as Record<string, unknown>);
    }
    if (this.opts.keplerLock) {
      // Phase 2: Kepler mode — WS к долгоиграющему Kepler host.
      return this.getKeplerTransport().request<T>(req);
    }
    // Use built-in child-process sidecar.
    return this.requestViaChild<T>(req);
  }

  private async requestAfterInit<T>(req: SidecarRequest): Promise<T> {
    await this.ensureInitialized();
    return this.request<T>(req);
  }

  // -------------------------------------------------------------------------
  // Internal: sidecar child-process management
  // -------------------------------------------------------------------------

  private getKeplerTransport(): ArkKeplerTransport {
    if (!this.opts.keplerLock) {
      throw new Error("@kosmos/ark: keplerLock is required for kepler mode");
    }
    if (!this.keplerTransport) {
      this.keplerTransport = new ArkKeplerTransport({
        lock: this.opts.keplerLock,
        deviceId: this.opts.deviceId,
        pidForHandshake: this.opts.keplerPidForHandshake,
        requestTimeoutMs: this.opts.requestTimeoutMs,
        onEvent: (event) => this.dispatchSidecarEvent(event),
      });
    }
    return this.keplerTransport;
  }

  private getChildTransport(): ArkChildTransport {
    if (!this.childTransport) {
      this.childTransport = new ArkChildTransport({
        sidecarPath: this.opts.sidecarPath,
        requestTimeoutMs: this.opts.requestTimeoutMs,
        onEvent: (event) => this.dispatchSidecarEvent(event),
      });
    }
    return this.childTransport;
  }

  private requestViaChild<T>(req: SidecarRequest): Promise<T> {
    return this.getChildTransport().request<T>(req);
  }

  private async ensureInitialized(): Promise<void> {
    if (this.delegateRequest || this.initialized) return;

    if (this.opts.keplerLock) {
      // Kepler host уже init'нул ark-core-rpc — нам не нужно слать init.
      // Просто гарантируем что WS-handshake завершён, после чего готовы.
      await this.getKeplerTransport().ensureConnected();
      this.initialized = true;
      return;
    }

    const dbPath = this.opts.dbPath;
    if (!dbPath) {
      throw new Error("@kosmos/ark: dbPath is required when requestFn is not provided");
    }

    fs.mkdirSync(path.dirname(dbPath), { recursive: true });
    await this.requestViaChild<boolean>({ operation: "init", dbPath });
    this.initialized = true;
  }

  private dispatchSidecarEvent(event: SidecarEvent): void {
    dispatchArkEvent(event, {
      arkEventCallbacks: this.arkEventCallbacks,
      entityChangedCallbacks: this.entityChangedCallbacks,
      peerConnectedCallbacks: this.peerConnectedCallbacks,
      peerDisconnectedCallbacks: this.peerDisconnectedCallbacks,
      commandInvokedCallbacks: this.commandInvokedCallbacks,
      commandsChangedCallbacks: this.commandsChangedCallbacks,
    });
  }
}
