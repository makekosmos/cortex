import type {
  CommandInvokedCallback,
  CommandInvokedEvent,
  CommandManifest,
  CommandsChangedCallback,
  EntityChangedCallback,
  PeerConnectedCallback,
  PeerDisconnectedCallback,
  SidecarEvent,
} from "./ark-client.types.js";

export interface ArkEventDispatcherCallbacks {
  arkEventCallbacks: Set<(event: SidecarEvent) => void>;
  entityChangedCallbacks: Set<EntityChangedCallback>;
  peerConnectedCallbacks: Set<PeerConnectedCallback>;
  peerDisconnectedCallbacks: Set<PeerDisconnectedCallback>;
  commandInvokedCallbacks: Set<CommandInvokedCallback>;
  commandsChangedCallbacks: Set<CommandsChangedCallback>;
}

export function dispatchArkEvent(
  event: SidecarEvent,
  callbacks: ArkEventDispatcherCallbacks,
): void {
  // Generic subscribers first — получают ВСЕ события, в том числе те, у которых
  // нет типизированного callback'а (например, `sync_error`, `sync_replay`).
  for (const cb of callbacks.arkEventCallbacks) {
    try {
      cb(event);
    } catch {
      /* ignore */
    }
  }
  switch (event.event) {
    case "entity_changed": {
      const entityJson =
        typeof event.entity === "object"
          ? JSON.stringify(event.entity)
          : String(event.entity ?? "");
      for (const cb of callbacks.entityChangedCallbacks) {
        try {
          cb(entityJson);
        } catch {
          /* ignore */
        }
      }
      break;
    }
    case "peer_connected": {
      const deviceId = String(event.device_id ?? "");
      const deviceName = String(event.device_name ?? "");
      for (const cb of callbacks.peerConnectedCallbacks) {
        try {
          cb(deviceId, deviceName);
        } catch {
          /* ignore */
        }
      }
      break;
    }
    case "peer_disconnected": {
      const deviceId = String(event.device_id ?? "");
      const remaining = typeof event.remaining === "number" ? event.remaining : 0;
      for (const cb of callbacks.peerDisconnectedCallbacks) {
        try {
          cb(deviceId, remaining);
        } catch {
          /* ignore */
        }
      }
      break;
    }
    case "command_invoked": {
      const id = String(event.id ?? "");
      const rawParams = event.params;
      const params =
        rawParams && typeof rawParams === "object" && !Array.isArray(rawParams)
          ? (rawParams as Record<string, unknown>)
          : undefined;
      const invokerClientId =
        typeof event.invokerClientId === "string"
          ? event.invokerClientId
          : typeof event.invoker_client_id === "string"
            ? (event.invoker_client_id as string)
            : undefined;
      const payload: CommandInvokedEvent = {
        id,
        ...(params !== undefined ? { params } : {}),
        ...(invokerClientId !== undefined ? { invokerClientId } : {}),
      };
      for (const cb of callbacks.commandInvokedCallbacks) {
        try {
          cb(payload);
        } catch {
          /* ignore */
        }
      }
      break;
    }
    case "commands_changed": {
      const list = Array.isArray(event.commands) ? (event.commands as CommandManifest[]) : [];
      for (const cb of callbacks.commandsChangedCallbacks) {
        try {
          cb(list);
        } catch {
          /* ignore */
        }
      }
      break;
    }
    default:
      break;
  }
}
