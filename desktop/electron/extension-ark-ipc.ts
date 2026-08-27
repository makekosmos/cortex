import { ipcMain, type WebContents } from "electron";
import {
  assertExtensionArkPermission,
  assertExtensionEventPermission,
  type ExtensionSource,
  type JsonRecord,
  type JsonValue,
} from "./extension-permissions";
import { applyAuthoritativeFocusState } from "./focus-enforcement";

export type ArkRequestFn = (req: JsonRecord) => Promise<JsonValue>;
export type ArkSubscribeFn = (event: string, handler: (payload: JsonValue) => void) => () => void;

interface ExtensionArkContext {
  id: string;
  source: ExtensionSource;
  manifestPermissions?: readonly string[];
}

interface ExtensionArkIpcOptions {
  contextForSender(sender: WebContents): ExtensionArkContext;
}

interface ObjectTypeResponse {
  typeId?: string;
  type_id?: string;
}

function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}

let arkRequest: ArkRequestFn | null = null;
let arkSubscribe: ArkSubscribeFn | null = null;
let arkBridgeReady!: Promise<void>;
let arkBridgeReadyResolve: (() => void) | null = null;
let arkBridgeReadyTimeoutMs = 15_000;

function resetArkBridgeReady(): void {
  arkBridgeReady = new Promise<void>((resolve) => {
    arkBridgeReadyResolve = resolve;
  });
}
resetArkBridgeReady();

async function awaitArkBridgeReady(timeoutMs = arkBridgeReadyTimeoutMs): Promise<void> {
  if (arkRequest) return;
  await Promise.race([
    arkBridgeReady,
    new Promise<void>((_, reject) =>
      setTimeout(() => reject(new Error("ark bridge not ready (timeout)")), timeoutMs),
    ),
  ]);
}

export function setExtensionArkBridgeReadyTimeoutMs(timeoutMs: number): void {
  if (Number.isFinite(timeoutMs) && timeoutMs > 0) {
    arkBridgeReadyTimeoutMs = timeoutMs;
  }
}

export function setExtensionArkBridge(opts: {
  request: ArkRequestFn | null;
  subscribe: ArkSubscribeFn | null;
}): void {
  arkRequest = opts.request;
  arkSubscribe = opts.subscribe;
  if (opts.request) {
    arkBridgeReadyResolve?.();
  } else {
    resetArkBridgeReady();
  }
}

async function applyExtensionFocusState(
  params: JsonRecord | undefined,
  request: ArkRequestFn,
): Promise<JsonValue> {
  const result = await applyAuthoritativeFocusState({
    request: (operation, nextParams) => {
      // SAFETY: focus-enforcement only emits JSON-safe request parameters.
      return request({ operation, ...(nextParams as JsonRecord) });
    },
    active: params?.active === true,
    blocklistId: isString(params?.blocklist_id) ? params.blocklist_id : null,
    blockedAppIds: Array.isArray(params?.blocked_app_ids)
      ? params.blocked_app_ids.filter(isString)
      : [],
    // SAFETY: extension IPC parameters are validated JSON values.
    blockedApps: Array.isArray(params?.blocked_apps) ? (params.blocked_apps as JsonValue[]) : [],
  });
  // SAFETY: applyAuthoritativeFocusState returns the JSON result of ArkRequestFn.
  return result.result as JsonValue;
}

const extensionEventUnsubscribers = new Map<string, () => void>();

export function registerExtensionArkIpc({ contextForSender }: ExtensionArkIpcOptions): void {
  ipcMain.handle(
    "kepler:extension:ark:request",
    async (e, operation: string, params?: JsonRecord) => {
      await awaitArkBridgeReady();
      const request = arkRequest;
      if (!request) {
        throw new Error("ark bridge not ready");
      }

      const context = contextForSender(e.sender);
      if (params && Object.prototype.hasOwnProperty.call(params, "operation")) {
        throw new Error("[kepler-shell] extension ARK params must not include operation");
      }
      await assertExtensionArkPermission({
        extensionId: context.id,
        source: context.source,
        manifestPermissions: context.manifestPermissions,
        operation,
        params,
        resolveObjectType: async (id) => {
          // SAFETY: get_object returns the documented object type metadata.
          const object = (await request({ operation: "get_object", id })) as
            | ObjectTypeResponse
            | null
            | undefined;
          const typeId = object?.typeId ?? object?.type_id;
          return isString(typeId) ? typeId : null;
        },
      });
      const req: JsonRecord = { ...params, operation };
      if (operation === "focus.set_active_state") {
        return applyExtensionFocusState(params, request);
      }
      return request(req);
    },
  );

  ipcMain.handle("kepler:extension:ark:subscribe", async (e, event: string) => {
    await awaitArkBridgeReady();
    if (!arkSubscribe) {
      throw new Error("ark bridge not ready");
    }
    const context = contextForSender(e.sender);
    assertExtensionEventPermission({
      extensionId: context.id,
      source: context.source,
      manifestPermissions: context.manifestPermissions,
      event,
    });
    const sender = e.sender;
    const key = `${sender.id}:${event}`;
    if (extensionEventUnsubscribers.has(key)) return true;

    const unsubscribe = arkSubscribe(event, (payload) => {
      if (!sender.isDestroyed()) {
        sender.send(`kepler:extension:ark:event:${event}`, payload);
      }
    });
    extensionEventUnsubscribers.set(key, unsubscribe);
    sender.once("destroyed", () => {
      const existing = extensionEventUnsubscribers.get(key);
      if (existing) {
        existing();
        extensionEventUnsubscribers.delete(key);
      }
    });
    return true;
  });

  ipcMain.handle("kepler:extension:ark:unsubscribe", (e, event: string) => {
    const key = `${e.sender.id}:${event}`;
    const unsubscribe = extensionEventUnsubscribers.get(key);
    if (unsubscribe) {
      unsubscribe();
      extensionEventUnsubscribers.delete(key);
    }
    return true;
  });
}
