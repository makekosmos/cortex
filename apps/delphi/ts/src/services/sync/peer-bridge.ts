/* eslint-disable no-console */

/**
 * Bridge between the renderer process and the Electron main-process PeerManager.
 *
 * Converts ArkChange objects to PeerChange format and sends them via IPC.
 * Only active when running inside Electron (window.electronAPI is available).
 */

import type { ArkChange } from "./ark-client";

import { HLC } from "./hlc";

const DEVICE_ID_KEY = "delphi.sync_device_id";

let localSeq = 0;

function getDeviceId(): string {
  return localStorage.getItem(DEVICE_ID_KEY) ?? "unknown";
}

/**
 * Broadcast an ArkChange to all P2P peers via the Electron main process.
 * No-op when not running in Electron.
 */

export function broadcastToPeers(change: ArkChange): void {
  if (!window.electronAPI?.invoke) return;

  const deviceId = getDeviceId();

  localSeq += 1;

  // Deep-clone to strip Vue 3 reactive proxies — Electron IPC structured clone

  // cannot serialize Proxy objects.

  const peerChange = JSON.parse(
    JSON.stringify({
      type: "change",

      event_id: change.event_id,

      change_type: change.change_type,

      data: change.data,

      origin_device: deviceId,

      origin_seq: localSeq,

      hlc: HLC.now(deviceId).toString(),

      hop_path: [deviceId],
    }),
  );

  window.electronAPI.invoke("peer:broadcastChange", peerChange).catch((err) => {
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
