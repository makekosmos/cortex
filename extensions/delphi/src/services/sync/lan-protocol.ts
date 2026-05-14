/**
 * LAN Protocol — inlined types and functions for P2P sync over WebSocket.
 *
 * Standalone implementation (arksync package removed).
 */

import { HLC } from "./hlc";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const LAN_SYNC_PORT = 21531;
export const PROTOCOL_VERSION = 1;
export const MAX_BATCH_SIZE = 100;
export const MAX_BATCH_BYTES = 1_048_576; // 1 MB
export const BATCH_ACK_TIMEOUT_MS = 10_000;
export const LIVE_ACK_TIMEOUT_MS = 5_000;
export const MAX_RETRIES = 3;
export const PING_INTERVAL_MS = 15_000;

// ---------------------------------------------------------------------------
// Entity types that sync
// ---------------------------------------------------------------------------

export type SyncEntityType = string;

export interface SyncEntity {
  type: SyncEntityType;
  id: string;
  data: Record<string, unknown>;
  hlc: string;
  deleted?: boolean;
}

// ---------------------------------------------------------------------------
// Version vector
// ---------------------------------------------------------------------------

export type VersionVector = Record<string, string>;

// ---------------------------------------------------------------------------
// Protocol messages
// ---------------------------------------------------------------------------

export interface HelloMessage {
  type: "hello";
  protocol_version: number;
  device_id: string;
  device_name: string;
  space_id: string;
  addresses?: string[];
}

export interface VersionVectorMessage {
  type: "version_vector";
  vector: VersionVector;
}

export interface SyncChangesMessage {
  type: "sync_changes";
  batch_id: string;
  entities: SyncEntity[];
  is_last: boolean;
}

export interface SyncAckMessage {
  type: "sync_ack";
  batch_id: string;
  accepted: number;
}

export interface LiveChangeMessage {
  type: "live_change";
  change_id: string;
  entity: SyncEntity;
}

export interface LiveAckMessage {
  type: "live_ack";
  change_id: string;
}

export interface PingMessage {
  type: "ping";
  ts: number;
}

export interface PongMessage {
  type: "pong";
  ts: number;
}

export interface PeerRecord {
  device_id: string;
  device_name: string;
  addresses: string[];
  last_seen: string;
  last_address?: string;
}

export interface PeerListMessage {
  type: "peer_list";
  peers: PeerRecord[];
}

export type LanSyncMessage =
  | HelloMessage
  | VersionVectorMessage
  | SyncChangesMessage
  | SyncAckMessage
  | LiveChangeMessage
  | LiveAckMessage
  | PingMessage
  | PongMessage
  | PeerListMessage;

// ---------------------------------------------------------------------------
// HLC comparison
// ---------------------------------------------------------------------------

export function compareHlc(a: string, b: string): number {
  const hlcA = HLC.fromString(a);
  const hlcB = HLC.fromString(b);
  return hlcA.compareTo(hlcB);
}

export function isNewerHlc(a: string, b: string): boolean {
  return compareHlc(a, b) > 0;
}

// ---------------------------------------------------------------------------
// Version vector diff
// ---------------------------------------------------------------------------

export function computeVectorDiff(
  local: VersionVector,
  remote: VersionVector,
): Set<string> {
  const needed = new Set<string>();

  for (const [entityId, remoteHlc] of Object.entries(remote)) {
    const localHlc = local[entityId];
    if (!localHlc) {
      needed.add(entityId);
    } else if (isNewerHlc(remoteHlc, localHlc)) {
      needed.add(entityId);
    }
  }

  return needed;
}

export function computeLocalExcess(
  local: VersionVector,
  remote: VersionVector,
): Set<string> {
  return computeVectorDiff(remote, local);
}

// ---------------------------------------------------------------------------
// Batch splitting
// ---------------------------------------------------------------------------

export function splitIntoBatches(entities: SyncEntity[]): SyncEntity[][] {
  if (entities.length === 0) return [];

  const batches: SyncEntity[][] = [];
  let current: SyncEntity[] = [];
  let currentBytes = 0;

  for (const entity of entities) {
    const entityBytes = JSON.stringify(entity).length;

    if (
      current.length > 0 &&
      (current.length >= MAX_BATCH_SIZE ||
        currentBytes + entityBytes > MAX_BATCH_BYTES)
    ) {
      batches.push(current);
      current = [];
      currentBytes = 0;
    }

    current.push(entity);
    currentBytes += entityBytes;
  }

  if (current.length > 0) {
    batches.push(current);
  }

  return batches;
}

// ---------------------------------------------------------------------------
// Message serialization
// ---------------------------------------------------------------------------

export function serializeMessage(msg: LanSyncMessage): string {
  return JSON.stringify(msg);
}

export function deserializeMessage(raw: string): LanSyncMessage | null {
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || !parsed.type) {
      return null;
    }
    return parsed as LanSyncMessage;
  } catch {
    return null;
  }
}

export function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// ---------------------------------------------------------------------------
// Peer record merging
// ---------------------------------------------------------------------------

export function mergePeerRecords(
  existing: PeerRecord[],
  incoming: PeerRecord[],
): PeerRecord[] {
  const map = new Map<string, PeerRecord>();

  for (const peer of existing) {
    map.set(peer.device_id, { ...peer });
  }

  for (const inc of incoming) {
    const current = map.get(inc.device_id);
    if (!current) {
      map.set(inc.device_id, { ...inc });
      continue;
    }

    // Merge addresses (union, deduplicated)
    const addrSet = new Set([...current.addresses, ...inc.addresses]);
    const useIncoming = inc.last_seen >= current.last_seen;
    const merged: PeerRecord = {
      ...current,
      addresses: [...addrSet],
      last_seen: useIncoming ? inc.last_seen : current.last_seen,
      device_name: useIncoming ? inc.device_name : current.device_name,
    };

    if (inc.last_address && useIncoming) {
      merged.last_address = inc.last_address;
    }

    map.set(inc.device_id, merged);
  }

  return [...map.values()];
}
