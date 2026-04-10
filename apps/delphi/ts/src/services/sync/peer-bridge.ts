/* eslint-disable no-console */

/**
 * Bridge between the renderer process and the Electron main-process PeerManager.
 *
 * Converts ArkChange objects to PeerChange format and sends them via IPC.
 * Only active when running inside Electron (window.electronAPI is available).
 */

import type { ArkChange } from "./ark-types";
import { arkChangeEventType } from "./ark-types";

const DEVICE_ID_KEY = "delphi.sync_device_id";

type SyncEntity = {
  type: string;
  id: string;
  data: Record<string, unknown>;
  hlc: string;
  deleted?: boolean;
};

function getDeviceId(): string {
  return localStorage.getItem(DEVICE_ID_KEY) ?? "unknown";
}

/**
 * Broadcast an ArkChange to all P2P peers via the Electron main process.
 * No-op when not running in Electron.
 */

export function broadcastToPeers(change: ArkChange): void {
  if (!window.electronAPI?.invoke) return;

  const eventType = arkChangeEventType(change);
  const entityType =
    eventType === "project"
      ? "project"
      : eventType === "task" || eventType === "task_created"
        ? "todo"
        : null;

  if (!entityType) return;

  const envelope = change.data as Record<string, unknown> | undefined;
  const entityId =
    (envelope?.source_id as string | undefined) ?? change.event_id ?? null;
  const entityData = (envelope?.data as Record<string, unknown> | undefined) ?? {};

  if (!entityId) return;

  // Keep a monotonic local counter for future debugging parity with the old
  // bridge, but the current lan-sync IPC contract no longer ships it.
  const peerChange: SyncEntity = JSON.parse(
    JSON.stringify({
      type: entityType,
      id: entityId.toLowerCase(),
      data: entityData,
      hlc: "",
      deleted: change.change_type === "delete" ? true : undefined,
    }),
  );

  window.electronAPI.invoke("lan-sync:broadcastChange", peerChange).catch((err) => {
    console.warn("[PeerBridge] Failed to broadcast change:", err);
  });
}

/**
 * Set up mesh credentials in the main process from an Ark API key.
 * This triggers PeerManager start/restart.
 */

export function setupMeshFromArkKey(apiKey: string): void {
  if (!window.electronAPI?.invoke) return;

  const deviceId = getDeviceId();

  window.electronAPI

    .invoke("peer:setMeshCredentials", apiKey, deviceId, "")

    .catch((err) => {
      console.warn("[PeerBridge] Failed to set mesh credentials:", err);
    });
}

/**
 * Set up mesh credentials in the main process from a space code.
 * The space code IS the mesh secret — same call, different source.
 */

export function setupMeshFromSpaceCode(code: string): void {
  if (!window.electronAPI?.invoke) return;

  const deviceId = getDeviceId();

  // Use the raw code (no dashes) as the mesh secret

  const meshSecret = code.replace(/[-\s]/g, "").toUpperCase();

  window.electronAPI

    .invoke("peer:setMeshCredentials", meshSecret, deviceId, "")

    .catch((err) => {
      console.warn(
        "[PeerBridge] Failed to set mesh credentials from space code:",

        err,
      );
    });
}

/** Stop the P2P mesh (called when leaving a space). */

export function stopMesh(): void {
  if (!window.electronAPI?.invoke) return;

  window.electronAPI.invoke("peer:stop").catch(() => {});
}
